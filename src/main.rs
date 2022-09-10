use crate::log_file_pattern::parse_log_file_pattern_from_env;

mod log_file_pattern;

fn main() {
	let log_file_pattern = match parse_log_file_pattern_from_env() {
		Ok(pattern) => pattern,
		Err(error) => {
			println!("Error: {}", error);
			return;
		}
	};
	
	let log_files = match log_file_pattern.search() {
		Ok(files) => files,
		Err(error) => {
			println!("Error searching log files: {}", error);
			return;
		}
	};
	
	for log_file in log_files {
		println!("Found log file: {} (label \"{}\")", log_file.path.display(), log_file.label);
	}
}
