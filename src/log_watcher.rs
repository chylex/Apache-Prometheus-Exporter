use std::cmp::max;
use std::path::PathBuf;
use std::sync::Arc;

use notify::{Event, EventKind};
use notify::event::{CreateKind, ModifyKind};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

use crate::ApacheMetrics;
use crate::fs_watcher::{FsEventCallbacks, FsWatcher};
use crate::log_file_pattern::LogFilePath;

#[derive(Copy, Clone, PartialEq)]
enum LogFileKind {
	Access,
	Error,
}

struct LogFileMetadata {
	pub kind: LogFileKind,
	pub label: String,
}

impl LogFileMetadata {
	fn get_label_set(&self) -> [(&'static str, String); 1] {
		[("file", self.label.clone())]
	}
}

pub async fn start_log_watcher(access_log_files: Vec<LogFilePath>, error_log_files: Vec<LogFilePath>, metrics: ApacheMetrics) -> bool {
	let mut watcher = LogWatcherConfiguration::new();
	
	for log_file in access_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Access);
	}
	
	for log_file in error_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Error);
	}
	
	watcher.start(&metrics).await
}

struct LogWatcherConfiguration {
	files: Vec<(PathBuf, LogFileMetadata)>,
}

impl LogWatcherConfiguration {
	fn new() -> LogWatcherConfiguration {
		LogWatcherConfiguration { files: Vec::new() }
	}
	
	fn count_files_of_kind(&self, kind: LogFileKind) -> usize {
		return self.files.iter().filter(|(_, metadata)| metadata.kind == kind).count();
	}
	
	fn add_file(&mut self, log_file: LogFilePath, kind: LogFileKind) {
		let path = log_file.path;
		let label = log_file.label;
		let metadata = LogFileMetadata { kind, label };
		self.files.push((path, metadata));
	}
	
	async fn start(self, metrics: &ApacheMetrics) -> bool {
		if self.files.is_empty() {
			println!("[LogWatcher] No log files provided.");
			return false;
		}
		
		println!("[LogWatcher] Watching {} access log file(s) and {} error log file(s).", self.count_files_of_kind(LogFileKind::Access), self.count_files_of_kind(LogFileKind::Error));
		
		struct PreparedFile {
			path: PathBuf,
			metadata: LogFileMetadata,
			fs_event_receiver: Receiver<Event>,
		}
		
		let mut prepared_files = Vec::new();
		let mut fs_callbacks = FsEventCallbacks::new();
		
		for (path, metadata) in self.files {
			let (fs_event_sender, fs_event_receiver) = mpsc::channel(20);
			fs_callbacks.register(&path, fs_event_sender);
			prepared_files.push(PreparedFile { path, metadata, fs_event_receiver });
		}
		
		let fs_watcher = match FsWatcher::new(fs_callbacks) {
			Ok(fs_watcher) => fs_watcher,
			Err(e) => {
				println!("[LogWatcher] Error creating filesystem watcher: {}", e);
				return false;
			}
		};
		
		for file in &prepared_files {
			let file_path = &file.path;
			if !file_path.is_absolute() {
				println!("[LogWatcher] Error creating filesystem watcher, path is not absolute: {}", file_path.to_string_lossy());
				return false;
			}
			
			let parent_path = if let Some(parent) = file_path.parent() {
				parent
			} else {
				println!("[LogWatcher] Error creating filesystem watcher for parent directory of file \"{}\", parent directory does not exist", file_path.to_string_lossy());
				return false;
			};
			
			if let Err(e) = fs_watcher.watch(parent_path).await {
				println!("[LogWatcher] Error creating filesystem watcher for directory \"{}\": {}", parent_path.to_string_lossy(), e);
				return false;
			}
		}
		
		let fs_watcher = Arc::new(fs_watcher);
		
		for file in prepared_files {
			let label_set = file.metadata.get_label_set();
			let _ = metrics.requests_total.get_or_create(&label_set);
			let _ = metrics.errors_total.get_or_create(&label_set);
			
			let log_watcher = match LogWatcher::create(file.path, file.metadata, metrics.clone(), Arc::clone(&fs_watcher), file.fs_event_receiver).await {
				Some(log_watcher) => log_watcher,
				None => return false,
			};
			
			tokio::spawn(log_watcher.watch());
		}
		
		true
	}
}

struct LogWatcher {
	state: LogWatchingState,
	processor: LogLineProcessor,
	fs_event_receiver: Receiver<Event>,
}

impl LogWatcher {
	async fn create(path: PathBuf, metadata: LogFileMetadata, metrics: ApacheMetrics, fs_watcher: Arc<FsWatcher>, fs_event_receiver: Receiver<Event>) -> Option<Self> {
		let state = match LogWatchingState::initialize(path.clone(), fs_watcher).await {
			Some(state) => state,
			None => return None,
		};
		
		let processor = LogLineProcessor { path, metadata, metrics };
		Some(LogWatcher { state, processor, fs_event_receiver })
	}
	
	async fn watch(mut self) {
		while let Ok(Some(_)) = self.state.lines.next_line().await {
			// Skip lines that already existed.
		}
		
		let path = &self.processor.path;
		
		'read_loop:
		loop {
			if !self.processor.process_lines(&mut self.state.lines).await {
				break 'read_loop;
			}
			
			'event_loop:
			loop {
				let mut next_event = CoalescedFsEvent::None;
				
				match self.fs_event_receiver.recv().await {
					None => break 'read_loop,
					Some(event) => {
						next_event = next_event.merge(event);
						
						while let Ok(event) = self.fs_event_receiver.try_recv() {
							next_event = next_event.merge(event);
						}
					}
				}
				
				match next_event {
					CoalescedFsEvent::None => continue 'event_loop,
					CoalescedFsEvent::NewData => continue 'read_loop,
					CoalescedFsEvent::NewFile => {
						println!("[LogWatcher] File recreated: {}", path.to_string_lossy());
						
						if !self.processor.process_lines(&mut self.state.lines).await {
							break 'read_loop;
						}
						
						self.state = match self.state.reinitialize().await {
							Some(state) => state,
							None => break 'read_loop,
						};
						
						continue 'read_loop;
					}
				}
			}
		}
		
		println!("[LogWatcher] Stopping log watcher for: {}", path.to_string_lossy());
	}
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum CoalescedFsEvent {
	None = 0,
	NewData = 1,
	NewFile = 2,
}

impl CoalescedFsEvent {
	fn merge(self, event: Event) -> CoalescedFsEvent {
		match event.kind {
			EventKind::Modify(ModifyKind::Data(_)) => {
				max(self, CoalescedFsEvent::NewData)
			}
			
			EventKind::Create(CreateKind::File) => {
				max(self, CoalescedFsEvent::NewFile)
			}
			
			_ => self
		}
	}
}

struct LogWatchingState {
	path: PathBuf,
	lines: Lines<BufReader<File>>,
	fs_watcher: Arc<FsWatcher>,
}

impl LogWatchingState {
	const DEFAULT_BUFFER_CAPACITY: usize = 1024 * 4;
	
	async fn initialize(path: PathBuf, fs_watcher: Arc<FsWatcher>) -> Option<LogWatchingState> {
		if let Err(e) = fs_watcher.watch(&path).await {
			println!("[LogWatcher] Error creating filesystem watcher for file \"{}\": {}", path.to_string_lossy(), e);
			return None;
		}
		
		let lines = match File::open(&path).await {
			Ok(file) => BufReader::with_capacity(Self::DEFAULT_BUFFER_CAPACITY, file).lines(),
			Err(e) => {
				println!("[LogWatcher] Error opening file \"{}\": {}", path.to_string_lossy(), e);
				return None;
			}
		};
		
		Some(LogWatchingState { path, lines, fs_watcher })
	}
	
	async fn reinitialize(self) -> Option<LogWatchingState> {
		LogWatchingState::initialize(self.path, self.fs_watcher).await
	}
}

struct LogLineProcessor {
	path: PathBuf,
	metadata: LogFileMetadata,
	metrics: ApacheMetrics,
}

impl LogLineProcessor {
	async fn process_lines(&self, reader: &mut Lines<BufReader<File>>) -> bool {
		loop {
			match reader.next_line().await {
				Ok(maybe_line) => match maybe_line {
					Some(line) => self.handle_line(line),
					None => return true,
				},
				Err(e) => {
					println!("[LogWatcher] Error reading from file \"{}\": {}", self.path.to_string_lossy(), e);
					return false;
				}
			}
		}
	}
	
	fn handle_line(&self, line: String) {
		let (kind, family) = match self.metadata.kind {
			LogFileKind::Access => ("access log", &self.metrics.requests_total),
			LogFileKind::Error => ("error log", &self.metrics.errors_total),
		};
		
		println!("[LogWatcher] Received {} line from \"{}\": {}", kind, self.metadata.label, line);
		family.get_or_create(&self.metadata.get_label_set()).inc();
	}
}
