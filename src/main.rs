// Taken from https://github.com/tauri-apps/tauri/blob/1aba10963e814163b9249ff2b6ad0f06b05d4a39/core/tauri/src/api/process/command.rs
// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::io::{BufRead, BufReader, Write};
use std::process::{Command};
use std::sync::{Arc, RwLock};
use std::thread::spawn;
use os_pipe::{pipe, PipeReader, PipeWriter};
use anyhow::Result;
use shared_child::SharedChild;
use tokio::sync::broadcast::{channel, Sender, Receiver};

#[derive(Clone, Debug)]
enum CommandEvent {
    Stderr(String),
    Stdout(String),
    Error(String),
    Terminated
}

fn spawn_pipe_reader<F: Fn(String) -> CommandEvent + Send + Copy + 'static>(
    tx: Sender<CommandEvent>,
    guard: Arc<RwLock<()>>,
    pipe_reader: PipeReader,
    wrapper: F
) {
    spawn(move || {
        let _lock = guard.read().unwrap();
        let mut reader = BufReader::new(pipe_reader);
        
        let mut buf = String::new();
        loop {
            buf.clear();
            match reader.read_line(&mut buf) { 
                Ok(n) => {
                    if n == 0 {
                        break;
                    }
                   
                    let tx_ = tx.clone();
               
                    let _ = tx_.send(wrapper(buf.clone())).unwrap();
                }
                Err(e)  => {
                    let tx_ =  tx.clone();
                    let _ = tx_.send(CommandEvent::Error(e.to_string())).unwrap();
                    break;
                }
            }
        } 
    });
}

fn spawn_sidecar() -> Result<(Receiver<CommandEvent>, Arc<SharedChild>, PipeWriter)> {
    let (stdout_reader, stdout_writer) = pipe()?;
    let (stderr_reader, stderr_writer) = pipe()?;
    let (stdin_reader, stdin_writer) = pipe()?;
    let mut command = Command::new("./sidecar");
    
    command.stdout(stdout_writer);
    command.stderr(stderr_writer);
    command.stdin(stdin_reader);
  
    let shared_child = SharedChild::spawn(&mut command)?;
    let child = Arc::new(shared_child);
    let child_ = child.clone();
    let guard = Arc::new(RwLock::new(()));
  
    let (tx, rx)  = channel(1);
    
    spawn_pipe_reader(tx.clone(), guard.clone(), stdout_reader, CommandEvent::Stdout);
    spawn_pipe_reader(tx.clone(), guard.clone(), stderr_reader, CommandEvent::Stderr);
   
    spawn(move || {
        let _ = match child_.wait() { 
            Ok(_status) => {
                let _l = guard.write().unwrap();
                tx.send(CommandEvent::Terminated).unwrap();
            },
            Err(e) => {
                let _l = guard.write().unwrap();
                tx.send(CommandEvent::Error(e.to_string())).unwrap();
            }
        };
    });
    
    Ok((rx, child, stdin_writer))
}

#[tokio::main]
async fn main() -> Result<()> {
    let (mut rx, _child, mut writer) = spawn_sidecar()?;

    writer.write_all(b"hello\n").unwrap();
    while let Ok(event) = rx.recv().await {
        match event { 
            CommandEvent::Error(err) => eprintln!("!!!: {}", err),
            CommandEvent::Stdout(line) => {
                println!("out: {}", line);
                writer.write_all(line.as_bytes()).unwrap();
            },
            CommandEvent::Stderr(line) => eprintln!("err: {}", line),
            CommandEvent::Terminated => eprintln!("terminated"),
        } 
    }
    
    Ok(())
}