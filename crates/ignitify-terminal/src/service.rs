use std::{
    io::{Read, Write},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::sync::mpsc as tokio_mpsc;

use crate::{Result, TerminalError};

const DEFAULT_SIZE: PtySize = PtySize {
    rows: 24,
    cols: 80,
    pixel_width: 0,
    pixel_height: 0,
};
const MIN_COLUMNS: u16 = 20;
const MAX_COLUMNS: u16 = 500;
const MIN_ROWS: u16 = 5;
const MAX_ROWS: u16 = 200;

#[derive(Debug)]
pub enum TerminalEvent {
    Output(Vec<u8>),
    Exited,
    Unavailable,
}

#[derive(Clone, Default)]
pub struct TerminalService;

impl TerminalService {
    pub fn open(&self) -> Result<TerminalSession> {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = tokio_mpsc::channel(128);

        thread::Builder::new()
            .name("ignitify-host-terminal".to_owned())
            .spawn(move || run_session(command_receiver, event_sender))
            .map_err(|_| TerminalError::Unavailable)?;

        Ok(TerminalSession {
            command_sender,
            event_receiver,
        })
    }
}

pub struct TerminalSession {
    command_sender: mpsc::Sender<TerminalCommand>,
    event_receiver: tokio_mpsc::Receiver<TerminalEvent>,
}

impl TerminalSession {
    pub fn input(&self, input: Vec<u8>) -> Result<()> {
        self.send(TerminalCommand::Input(input))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.send(TerminalCommand::Resize(PtySize {
            cols: cols.clamp(MIN_COLUMNS, MAX_COLUMNS),
            rows: rows.clamp(MIN_ROWS, MAX_ROWS),
            pixel_width: 0,
            pixel_height: 0,
        }))
    }

    pub async fn next_event(&mut self) -> Option<TerminalEvent> {
        self.event_receiver.recv().await
    }

    pub fn close(&self) {
        let _ = self.command_sender.send(TerminalCommand::Close);
    }

    fn send(&self, command: TerminalCommand) -> Result<()> {
        self.command_sender
            .send(command)
            .map_err(|_| TerminalError::Closed)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.close();
    }
}

enum TerminalCommand {
    Input(Vec<u8>),
    Resize(PtySize),
    Close,
}

fn run_session(
    commands: mpsc::Receiver<TerminalCommand>,
    events: tokio_mpsc::Sender<TerminalEvent>,
) {
    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(DEFAULT_SIZE) {
        Ok(pair) => pair,
        Err(_) => {
            emit(&events, TerminalEvent::Unavailable);
            return;
        }
    };
    let mut child = match pair.slave.spawn_command(CommandBuilder::new_default_prog()) {
        Ok(child) => child,
        Err(_) => {
            emit(&events, TerminalEvent::Unavailable);
            return;
        }
    };
    drop(pair.slave);

    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(_) => {
            emit(&events, TerminalEvent::Unavailable);
            let _ = child.kill();
            return;
        }
    };
    let reader_events = events.clone();
    let _ = thread::Builder::new()
        .name("ignitify-host-terminal-output".to_owned())
        .spawn(move || read_output(&mut reader, reader_events));

    let mut writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(_) => {
            emit(&events, TerminalEvent::Unavailable);
            let _ = child.kill();
            return;
        }
    };

    loop {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(TerminalCommand::Input(input)) => {
                if writer
                    .write_all(&input)
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
            Ok(TerminalCommand::Resize(size)) => {
                if pair.master.resize(size).is_err() {
                    break;
                }
            }
            Ok(TerminalCommand::Close) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        match child.try_wait() {
            Ok(Some(_)) => {
                emit(&events, TerminalEvent::Exited);
                return;
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }

    let _ = child.kill();
    emit(&events, TerminalEvent::Exited);
}

fn emit(events: &tokio_mpsc::Sender<TerminalEvent>, event: TerminalEvent) -> bool {
    events.blocking_send(event).is_ok()
}

fn read_output(reader: &mut (dyn Read + Send), events: tokio_mpsc::Sender<TerminalEvent>) {
    let mut buffer = [0_u8; 8192];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(count) => count,
        };
        if !emit(&events, TerminalEvent::Output(buffer[..count].to_vec())) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_COLUMNS, MAX_ROWS, MIN_COLUMNS, MIN_ROWS, TerminalCommand, TerminalSession};

    #[test]
    fn resize_limits_host_terminal_dimensions() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (_, events) = tokio::sync::mpsc::channel(1);
        let session = TerminalSession {
            command_sender: sender,
            event_receiver: events,
        };

        session.resize(1, u16::MAX).unwrap();

        let TerminalCommand::Resize(size) = receiver.recv().unwrap() else {
            panic!("resize command expected");
        };
        assert_eq!((size.cols, size.rows), (MIN_COLUMNS, MAX_ROWS));

        session.resize(u16::MAX, 1).unwrap();
        let TerminalCommand::Resize(size) = receiver.recv().unwrap() else {
            panic!("resize command expected");
        };
        assert_eq!((size.cols, size.rows), (MAX_COLUMNS, MIN_ROWS));
    }
}
