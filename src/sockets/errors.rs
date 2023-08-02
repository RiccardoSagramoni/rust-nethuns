use thiserror::Error;

#[derive(Error, Debug)]
pub enum NethunsOpenError {
	#[error("[try_new] invalid options: {0}")]
	InvalidOptions(String),
    #[error("[try_new] allocation error: {0}")]
    AllocationError(String)
}

#[derive(Error, Debug)]
pub enum NethunsBindError {
	#[error("[bind] error of the I/O framework: {0}")]
	FrameworkError(String),
	#[error("[bind] error caused by an illegal or inappropiate argument: {0}")]
	IllegalArgument(String),
	#[error("[bind] error caused by nethuns: {0}")]
	NethunsError(String)
}
