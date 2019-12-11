//! # Handler
//!
//! Error/kill handler for sub threads and services

use std::thread;

use futures::sync::oneshot;

/// Handler struct responsible for sending a stop signal to a service and
/// joining a thread back to the main thread
pub struct Handle<'a> {
    /// Channel to send kill signal
    tx: oneshot::Sender<()>,
    /// Service thread handler
    thread: thread::JoinHandle<()>,
    /// Service name
    name: &'a str,
}

impl<'a> Handle<'a> {
    /// Return new handle instance
    pub fn new(tx: oneshot::Sender<()>, thread: thread::JoinHandle<()>, name: &str) -> Handle {
        Handle { tx, thread, name }
    }

    /// Handle sending a stop signal to the service and joining the service
    /// thread
    pub fn stop(self) {
        self.tx
            .send(())
            .expect(&format!("failed sending shutdown signal to {}", self.name));
        self.thread.join().expect(&format!("{} join failed", self.name));
    }
}
