//! The audio on a thread of its own, so that the window keeps drawing while devices open.

use std::{
    io, iter,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    vec,
};

use super::{AudioError, Caps, Dir, Engine, EngineStatus, Io, PerDir, Pick, device_names};
use crate::plugin::{Host, Shelf};

enum Request {
    /// Starts the engine on `io` as start `seq`, reading the devices of the directions in `reread`
    /// again.
    Start {
        seq: u64,
        io: Io,
        reread: PerDir<bool>,
    },
    /// Lists the devices.
    List,
}

/// What the audio thread did.
pub enum Reply {
    /// Start `seq` found the devices can do `caps`, and starts the engine on `io`: the settings it
    /// was sent, fitted to them.
    Fitted {
        seq: u64,
        caps: PerDir<Caps>,
        io: Io,
    },
    /// The last start sent, folded with the ones before it, started or failed to.
    Started {
        seq: u64,
        result: Result<EngineStatus, AudioError>,
    },
    /// The names of the devices, the picked ones included.
    Listed(PerDir<Vec<String>>),
}

/// The audio thread, which owns the engine and stops it when this is dropped.
pub struct Audio {
    requests: Option<Sender<Request>>,
    replies: Receiver<Reply>,
    /// What [`Self::replies`] hands out, kept to be filled again.
    received: Vec<Reply>,
    /// The last start sent.
    sent: u64,
    /// The last start answered.
    answered: u64,
    thread: Option<JoinHandle<()>>,
}

impl Audio {
    /// Starts the thread that runs the plugin of `host`, which calls `wake` whenever it replied
    /// and once it stopped; nothing plays before [`Self::start`].
    ///
    /// # Errors
    ///
    /// When the system cannot start the thread.
    pub fn spawn(host: Arc<Host>, wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        let (requests, inbox) = mpsc::channel();
        let (outbox, replies) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("audio control".to_owned())
            .spawn(move || {
                let outbox = Outbox {
                    replies: outbox,
                    wake,
                };

                serve(&host, &inbox, &outbox);
                (outbox.wake)();
            })?;

        Ok(Self {
            requests: Some(requests),
            replies,
            received: Vec::new(),
            sent: 0,
            answered: 0,
            thread: Some(thread),
        })
    }

    /// Sends `request`; a thread that is gone shows in [`Self::replies`].
    fn send(&self, request: Request) {
        if let Some(requests) = &self.requests {
            requests.send(request).ok();
        }
    }

    /// Restarts the engine on `io` fitted to what its devices can do, reading them again where
    /// `reread` holds or where the thread last read another device.
    pub fn start(&mut self, io: Io, reread: PerDir<bool>) {
        self.sent += 1;
        self.send(Request::Start {
            seq: self.sent,
            io,
            reread,
        });
    }

    /// Lists the devices again, answered by [`Reply::Listed`].
    pub fn list(&self) {
        self.send(Request::List);
    }

    /// Whether a start is still under way.
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.sent != self.answered
    }

    /// What the thread did since the last call, without the replies to starts sent over since; a
    /// failed start if it stopped with one under way.
    pub fn replies(&mut self) -> vec::Drain<'_, Reply> {
        let replies = &mut self.received;
        let stopped = loop {
            match self.replies.try_recv() {
                Ok(reply) => replies.push(reply),
                Err(TryRecvError::Empty) => break false,
                Err(TryRecvError::Disconnected) => break true,
            }
        };

        self.answered = replies
            .iter()
            .filter_map(|it| match it {
                Reply::Started { seq, .. } => Some(*seq),
                Reply::Fitted { .. } | Reply::Listed(_) => None,
            })
            .fold(self.answered, u64::max);
        replies.retain(|it| match it {
            Reply::Fitted { seq, .. } | Reply::Started { seq, .. } => *seq == self.sent,
            Reply::Listed(_) => true,
        });

        if stopped && self.sent != self.answered {
            self.answered = self.sent;
            replies.push(Reply::Started {
                seq: self.sent,
                result: Err(AudioError::ThreadStopped),
            });
        }

        replies.drain(..)
    }
}

impl Drop for Audio {
    /// Waits for the thread to stop the engine, which cleans up the plugin.
    fn drop(&mut self) {
        drop(self.requests.take());

        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

/// What the device last read in each direction can do, by the pick it was read for; none before
/// the first read.
#[derive(Default)]
struct Known(PerDir<Option<(Pick, Caps)>>);

impl Known {
    /// What the device `pick` names in `dir` can do: as last read, unless that was another device
    /// or `again` holds, then as `probe` reads it now.
    fn read(&mut self, dir: Dir, pick: &Pick, again: bool, probe: impl FnOnce() -> Caps) -> &Caps {
        let known = &mut self.0[dir];

        if again || known.as_ref().is_none_or(|(read, _)| read != pick) {
            *known = None;
        }

        &known.get_or_insert_with(|| (pick.clone(), probe())).1
    }

    /// The device names of both directions, with the devices read last.
    fn listed(&self) -> PerDir<Vec<String>> {
        PerDir::from_fn(|dir| {
            let mut names = device_names(dir);

            if let Some((Pick::Device(name), _)) = &self.0[dir]
                && !names.contains(name)
            {
                names.push(name.clone());
            }

            names
        })
    }
}

/// Where the thread replies, and how it wakes the window to take the reply.
struct Outbox<W> {
    replies: Sender<Reply>,
    wake: W,
}

impl<W: Fn()> Outbox<W> {
    /// Sends `reply`; whether the window is still there to take it.
    fn send(&self, reply: Reply) -> bool {
        let sent = self.replies.send(reply).is_ok();

        if sent {
            (self.wake)();
        }

        sent
    }
}

/// Answers the requests queued behind each other at once, starting only the last start; returns
/// once the window has gone.
fn serve(host: &Host, requests: &Receiver<Request>, replies: &Outbox<impl Fn()>) {
    // Dropped after the engine, so the instance cleans up on this thread.
    let shelf = Shelf::default();
    let mut engine = None;
    let mut known = Known::default();

    while let Ok(first) = requests.recv() {
        let (list, start) = iter::once(first).chain(requests.try_iter()).fold(
            (false, None::<(u64, Io, PerDir<bool>)>),
            |(list, start), request| match request {
                Request::Start { seq, io, reread } => {
                    let earlier = start.map_or_default(|(_, _, earlier)| earlier);

                    (
                        list,
                        Some((seq, io, PerDir::from_fn(|dir| reread[dir] || earlier[dir]))),
                    )
                }
                Request::List => (true, start),
            },
        );

        if let Some((seq, io, reread)) = start {
            drop(engine.take());

            let caps = PerDir::from_fn(|dir| {
                io.get(dir).map_or_default(|settings| {
                    let pick = settings.pick();

                    known
                        .read(dir, pick, reread[dir], || Caps::probe(dir, pick))
                        .clone()
                })
            });
            let io = io.fitted(&caps);
            let fitted = Reply::Fitted {
                seq,
                caps: caps.clone(),
                io: io.clone(),
            };

            if !replies.send(fitted) {
                return;
            }

            let result = Engine::start(host, &shelf, &io, &caps).map(|(started, status)| {
                engine = Some(started);
                status
            });

            if !replies.send(Reply::Started { seq, result }) {
                return;
            }
        }

        if list && !replies.send(Reply::Listed(known.listed())) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use super::*;

    #[test]
    fn a_device_is_read_again_only_when_another_was_read_or_a_retry_asks() {
        let reads = Cell::new(0);
        let probe = || {
            reads.set(reads.get() + 1);
            Caps::of_ranges(&[])
        };
        let other = Pick::Device("other".to_owned());
        let mut known = Known::default();

        known.read(Dir::Input, &Pick::Default, false, probe);
        known.read(Dir::Input, &Pick::Default, false, probe);
        known.read(Dir::Output, &Pick::Default, false, probe);
        assert_eq!(reads.get(), 2);

        known.read(Dir::Output, &other, false, probe);
        known.read(Dir::Output, &other, false, probe);
        assert_eq!(reads.get(), 3);

        known.read(Dir::Output, &other, true, probe);
        assert_eq!(reads.get(), 4);
    }
}
