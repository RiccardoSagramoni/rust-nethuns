use thiserror::Error;

#[derive(Error, Debug)]
pub enum NethunsOpenError {
	#[error("[open] invalid options: {0}")]
	InvalidOptions(String)
}
