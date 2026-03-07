pub mod manager;
pub mod session;

pub use manager::PersistentShellManager;
pub use session::PersistentShell;

#[cfg(test)]
mod tests;
