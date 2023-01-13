use std::fmt::{Display, Error, Formatter};

pub struct AccessLogLineParts<'a> {
	pub time: &'a str,
	pub remote_host: &'a str,
	pub request: &'a str,
	pub response_status: &'a str,
	pub response_bytes: &'a str,
	pub response_time_ms: &'a str,
	pub referer: &'a str,
	pub user_agent: &'a str,
}

impl Display for AccessLogLineParts<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
		write!(f, "[{}] {} \"{}\" {} {} {} \"{}\" \"{}\"", self.time, self.remote_host, self.request, self.response_status, self.response_bytes, self.response_time_ms, self.referer, self.user_agent)
	}
}

impl<'a> AccessLogLineParts<'a> {
	pub fn parse(line: &'a str) -> Result<AccessLogLineParts<'a>, ParseError> {
		let (time, line) = extract_between_chars(line, '[', ']').ok_or(ParseError::TimeBracketsNotFound)?;
		let (remote_host, line) = next_space_delimited_part(line).ok_or(ParseError::RemoteHostNotFound)?;
		let (request, line) = extract_between_chars(line.trim_start_matches(' '), '"', '"').ok_or(ParseError::RequestNotFound)?;
		let (response_status, line) = next_space_delimited_part(line).ok_or(ParseError::ResponseStatusNotFound)?;
		let (response_bytes, line) = next_space_delimited_part(line).ok_or(ParseError::ResponseBytesNotFound)?;
		let (response_time_ms, line) = next_space_delimited_part(line).ok_or(ParseError::ResponseTimeNotFound)?;
		let (referer, line) = extract_between_chars(line.trim_start_matches(' '), '"', '"').ok_or(ParseError::RefererNotFound)?;
		let (user_agent, _) = extract_between_chars(line.trim_start_matches(' '), '"', '"').ok_or(ParseError::UserAgentNotFound)?;
		Ok(AccessLogLineParts { time, remote_host, request, response_status, response_bytes, response_time_ms, referer, user_agent })
	}
}

fn next_space_delimited_part(str: &str) -> Option<(&str, &str)> {
	return str.trim_start_matches(' ').split_once(' ')
}

fn extract_between_chars(str: &str, left_side: char, right_side: char) -> Option<(&str, &str)> {
	let str = str.trim_start_matches(' ');
	let next_char = str.chars().next()?;
	return if next_char == left_side {
		str.get(1..)?.split_once(right_side)
	} else {
		None
	};
}

#[derive(Debug, Copy, Clone)]
pub enum ParseError {
	TimeBracketsNotFound,
	RemoteHostNotFound,
	RequestNotFound,
	ResponseStatusNotFound,
	ResponseBytesNotFound,
	ResponseTimeNotFound,
	RefererNotFound,
	UserAgentNotFound,
}
