use std::collections::HashMap;
use std::path::{Path, PathBuf};

use notify::{ErrorKind, Event, recommended_watcher, RecommendedWatcher, RecursiveMode, Result, Watcher};
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

pub struct FsWatcher {
	watcher: Mutex<RecommendedWatcher>,
}

impl FsWatcher {
	pub fn new(callbacks: FsEventCallbacks) -> Result<Self> {
		let watcher = recommended_watcher(move |event| callbacks.handle_event(event))?;
		let watcher = Mutex::new(watcher);
		
		Ok(Self { watcher })
	}
	
	pub async fn watch(&self, path: &Path) -> Result<()> {
		let mut watcher = self.watcher.lock().await;
		
		if let Err(e) = watcher.unwatch(path) {
			if !matches!(e.kind, ErrorKind::WatchNotFound) {
				return Err(e);
			}
		}
		
		watcher.watch(path, RecursiveMode::NonRecursive)
	}
}

pub struct FsEventCallbacks {
	senders: HashMap<PathBuf, Sender<Event>>,
}

impl FsEventCallbacks {
	pub fn new() -> Self {
		Self { senders: HashMap::new() }
	}
	
	pub fn register(&mut self, path: &Path, sender: Sender<Event>) {
		self.senders.insert(path.to_path_buf(), sender);
	}
	
	fn handle_event(&self, event: Result<Event>) {
		match event {
			Ok(event) => {
				for path in &event.paths {
					if let Some(sender) = self.senders.get(path) {
						if let Err(e) = sender.try_send(event.clone()) {
							println!("[FsWatcher] Error sending filesystem event for path \"{}\": {}", path.to_string_lossy(), e);
						}
					}
				}
			}
			Err(e) => {
				println!("[FsWatcher] Error receiving filesystem event: {}", e);
			}
		}
	}
}
