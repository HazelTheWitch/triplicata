use std::time::Duration;

use tokio::{select, time::Instant};
use tracing::debug;

use crate::{
    config::{Action, Config},
    cube::Move,
};

#[derive(Debug)]
pub struct StateMachine {
    reciever: tokio::sync::broadcast::Receiver<Move>,
    current_prefix: Vec<Move>,
    tentative_bind: Option<usize>,
    config: Config,
}

impl StateMachine {
    pub fn new(reciever: tokio::sync::broadcast::Receiver<Move>, config: Config) -> Self {
        Self {
            reciever,
            config,
            tentative_bind: None,
            current_prefix: Vec::new(),
        }
    }

    fn reset(&mut self, tx: &mut tokio::sync::mpsc::UnboundedSender<Action>) {
        if let Some(bind) = self.tentative_bind {
            self.play_bind(bind, tx);
        }

        self.current_prefix.clear();
        self.tentative_bind = None;
    }

    fn get_tentative_bind(&self) -> Option<usize> {
        for (i, bind) in self.config.binds.iter().enumerate() {
            if bind.trigger == self.current_prefix {
                return Some(i);
            }
        }

        None
    }

    fn play_bind(&self, bind: usize, tx: &mut tokio::sync::mpsc::UnboundedSender<Action>) {
        for action in &self.config.binds[bind].actions {
            tx.send(*action).expect("could not send action");
        }
    }

    fn push_move(&mut self, m: Move, tx: &mut tokio::sync::mpsc::UnboundedSender<Action>) {
        self.current_prefix.push(m);

        match self.get_tentative_bind() {
            Some(new_tentative) => self.tentative_bind = Some(new_tentative),
            None => {
                if let Some(bind) = self.tentative_bind {
                    self.play_bind(bind, tx);
                }

                self.current_prefix.drain(..(self.current_prefix.len() - 1));
            }
        }

        let mut next_tentative = None;
        let mut only_one = false;

        for (i, bind) in self.config.binds.iter().enumerate() {
            if bind.trigger.starts_with(&self.current_prefix) {
                if next_tentative.is_none() {
                    next_tentative = Some(i);
                    only_one = true;
                } else {
                    only_one = false;
                }
            }
        }

        self.tentative_bind = next_tentative;

        match (next_tentative, only_one) {
            (Some(_), true) => {
                self.reset(tx);
            }
            _ => {}
        }
    }

    pub fn run(mut self) -> tokio::sync::mpsc::UnboundedReceiver<Action> {
        let (mut tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let timeout = Duration::from_millis(self.config.timeout);

        let mut last_move = Instant::now();

        tokio::spawn(async move {
            loop {
                select! {
                    Ok(m) = self.reciever.recv() => {
                        self.push_move(m, &mut tx);
                    }
                    _ = tokio::time::sleep_until(last_move + timeout) => {
                        self.reset(&mut tx);
                    }
                }

                debug!("{:?} ({:?})", self.current_prefix, self.tentative_bind);
                last_move = Instant::now();
            }
        });

        rx
    }
}
