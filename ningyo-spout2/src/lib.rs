mod error;
mod name;
mod registry;
mod semaphore;
mod sender;
mod shm;

pub use error::RegisterError;
pub use name::MAX_SENDER_LEN;
pub use registry::SenderRegistry;
pub use sender::Registration;
