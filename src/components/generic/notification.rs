use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Success,
    Error,
}

const TTL: Duration = Duration::from_secs(3);
const ENTER: Duration = Duration::from_millis(180);
const EXIT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    kind: NotificationKind,
    title: String,
    message: String,
    elapsed: Duration,
}

impl Notification {
    pub fn success(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(NotificationKind::Success, title, message)
    }

    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(NotificationKind::Error, title, message)
    }

    pub fn kind(&self) -> NotificationKind {
        self.kind
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn tick(&mut self, dt: Duration) {
        self.elapsed = (self.elapsed + dt).min(TTL);
    }

    pub fn is_expired(&self) -> bool {
        self.elapsed >= TTL
    }

    pub fn is_animating(&self) -> bool {
        self.elapsed < ENTER || TTL.saturating_sub(self.elapsed) <= EXIT
    }

    pub fn slide_offset(&self, width: u16) -> u16 {
        let travel = width.saturating_add(2);
        if self.elapsed < ENTER {
            let progress = self.elapsed.as_secs_f64() / ENTER.as_secs_f64();
            let eased = ease_out_cubic(progress);
            ((1.0 - eased) * f64::from(travel)).ceil() as u16
        } else if TTL.saturating_sub(self.elapsed) <= EXIT {
            let elapsed_exit = EXIT.saturating_sub(TTL.saturating_sub(self.elapsed));
            let progress = elapsed_exit.as_secs_f64() / EXIT.as_secs_f64();
            let eased = ease_in_cubic(progress);
            (eased * f64::from(travel)).ceil() as u16
        } else {
            0
        }
    }

    fn new(kind: NotificationKind, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            message: message.into(),
            elapsed: Duration::ZERO,
        }
    }
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_cubic(t: f64) -> f64 {
    t.powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_expires_after_three_seconds() {
        let mut notification = Notification::success("Copied", "KAN-1");

        notification.tick(Duration::from_secs(2));
        assert!(!notification.is_expired());

        notification.tick(Duration::from_secs(1));
        assert!(notification.is_expired());
    }

    #[test]
    fn notification_slides_in_holds_then_slides_out() {
        let mut notification = Notification::success("Copied", "KAN-1");

        assert!(notification.is_animating());
        assert_eq!(notification.slide_offset(50), 52);

        notification.tick(ENTER);
        assert!(!notification.is_animating());
        assert_eq!(notification.slide_offset(50), 0);

        notification.tick(TTL - ENTER - EXIT);
        assert!(notification.is_animating());
        assert_eq!(notification.slide_offset(50), 0);
    }
}
