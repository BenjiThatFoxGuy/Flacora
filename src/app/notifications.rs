//! Toast notifications, auto-dismiss timers, and undo actions.

use super::App;
use crate::jobs::Job;
use crate::library::{self, Entry};
use rune::Tx;

#[derive(Clone, Copy)]
pub enum NotificationKind {
    Success,
    Warning,
    Error,
}

pub enum UndoAction {
    History(Vec<Entry>),
    FinishedJobs(Vec<Job>),
}

/// Auto-dismiss lifecycle for a toast notification.
enum Timer {
    Pending,
    Armed { deadline: f64 },
    Paused { remaining: f64 },
}

pub struct Notification {
    pub kind: NotificationKind,
    pub message: String,
    timer: Timer,
}

impl Notification {
    fn new(kind: NotificationKind, message: String) -> Self {
        Self { kind, message, timer: Timer::Pending }
    }

    /// Advances the timer by one frame; returns `true` once the notification expires.
    fn tick(&mut self, clock: f64, lifetime: f64, paused: bool, tx: &mut Tx) -> bool {
        if paused {
            self.timer = match self.timer {
                Timer::Pending => Timer::Paused { remaining: lifetime },
                Timer::Armed { deadline } => Timer::Paused { remaining: (deadline - clock).max(1.0) },
                Timer::Paused { remaining } => Timer::Paused { remaining },
            };
            return false;
        }
        let deadline = match self.timer {
            Timer::Pending => {
                tx.animated();
                let deadline = clock + lifetime;
                self.timer = Timer::Armed { deadline };
                deadline
            }
            Timer::Paused { remaining } => {
                let deadline = clock + remaining;
                self.timer = Timer::Armed { deadline };
                deadline
            }
            Timer::Armed { deadline } => deadline,
        };
        if clock >= deadline {
            tx.animated();
            true
        } else {
            tx.wake_at(deadline);
            false
        }
    }
}

impl App {
    pub fn success(&mut self, message: impl Into<String>) {
        self.discard_undo();
        self.notify(NotificationKind::Success, message.into());
    }

    pub fn success_undo(&mut self, message: impl Into<String>, undo: UndoAction) {
        self.discard_undo();
        self.undo = Some(undo);
        self.notify(NotificationKind::Success, message.into());
    }

    pub fn dismiss_info(&mut self) {
        self.info = None;
        self.discard_undo();
    }

    pub fn discard_undo(&mut self) {
        if let Some(UndoAction::History(_)) = self.undo.take() {
            let keep: Vec<String> = self.history.iter().filter_map(|e| e.cover.clone()).collect();
            self.images.retain(|path, _| keep.contains(path));
            self.image_order.retain(|path| self.images.contains_key(path));
            let paths = self.paths.clone();
            self.background_opt(move || {
                library::prune_unused_covers(&paths, &keep);
                None
            });
        }
    }

    pub fn undo_last(&mut self) {
        self.info = None;
        match self.undo.take() {
            Some(UndoAction::History(entries)) => {
                self.history = entries;
                self.save_history();
                self.load_history_covers();
            }
            Some(UndoAction::FinishedJobs(jobs)) => {
                self.jobs.extend(jobs);
                self.jobs.sort_by_key(|j| j.id);
            }
            None => {}
        }
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.notify(NotificationKind::Warning, message.into());
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.notify(NotificationKind::Error, message.into());
    }

    fn notify(&mut self, kind: NotificationKind, message: String) {
        let toast = Some(Notification::new(kind, message));
        match kind {
            NotificationKind::Success => self.info = toast,
            NotificationKind::Warning | NotificationKind::Error => self.notice = toast,
        }
        // An input handler can create a notification after the frame's poll.
        if let Some(link) = &self.link {
            (link.waker)();
        }
    }

    pub fn tick_notifications(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut expired = false;
        let info_lifetime = if self.undo.is_some() { 6.0 } else { 2.5 };
        if self.info.as_mut().is_some_and(|toast| toast.tick(clock, info_lifetime, self.toast_hovered, tx)) {
            self.info = None;
            expired = true;
            self.discard_undo();
        }
        if self.notice.as_mut().is_some_and(|toast| toast.tick(clock, 4.0, self.toast_hovered, tx)) {
            self.notice = None;
            expired = true;
        }
        if self.info.is_none() && self.notice.is_none() {
            self.toast_hovered = false;
        }
        expired
    }
}
