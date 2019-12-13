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
    /// Channel to receive error from service
    err_rx: Option<oneshot::Receiver<()>>,
    /// Service thread handler
    thread: thread::JoinHandle<()>,
    /// Service name
    name: &'a str,
}

impl<'a> Handle<'a> {
    /// Return new handle instance
    pub fn new(
        tx: oneshot::Sender<()>,
        err_rx: Option<oneshot::Receiver<()>>,
        thread: thread::JoinHandle<()>,
        name: &str,
    ) -> Handle {
        Handle {
            tx,
            err_rx,
            thread,
            name,
        }
    }

    /// Check if an err signal has been received in the error receiver channel
    pub fn got_err(&mut self) -> bool {
        if let Some(rcv) = &mut self.err_rx {
            if rcv.try_recv().expect("").is_some() {
                return true;
            }
        }
        false
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
