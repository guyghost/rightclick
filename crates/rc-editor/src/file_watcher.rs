use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::futures::{SinkExt, StreamExt};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WatchSignal {
    Changed { generation: u64 },
    Unavailable { generation: u64, message: String },
}

pub(crate) fn subscription(
    path: PathBuf,
    generation: u64,
) -> iced::Subscription<WatchSignal> {
    let identity = (path.clone(), generation);

    iced::Subscription::run_with_id(
        identity,
        iced::stream::channel(100, move |mut output| async move {
            let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded();
            let watcher = match watch_with(&path, generation, move |signal| {
                let _ = sender.unbounded_send(signal);
            }) {
                Ok(watcher) => watcher,
                Err(message) => {
                    let _ = output
                        .send(WatchSignal::Unavailable {
                            generation,
                            message,
                        })
                        .await;
                    return;
                }
            };

            while let Some(signal) = receiver.next().await {
                if output.send(signal).await.is_err() {
                    break;
                }
            }

            drop(watcher);
        }),
    )
}

pub(crate) fn watch_with<F>(
    target: &Path,
    generation: u64,
    mut on_signal: F,
) -> Result<Debouncer<RecommendedWatcher>, String>
where
    F: FnMut(WatchSignal) + Send + 'static,
{
    let target = target.to_path_buf();
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let event_target = target.clone();

    let mut debouncer = new_debouncer(
        Duration::from_millis(150),
        move |result: notify_debouncer_mini::DebounceEventResult| match result {
            Ok(events) => {
                if events
                    .iter()
                    .any(|event: &DebouncedEvent| path_matches_target(&event.path, &event_target))
                {
                    on_signal(WatchSignal::Changed { generation });
                }
            }
            Err(error) => on_signal(WatchSignal::Unavailable {
                generation,
                message: error.to_string(),
            }),
        },
    )
    .map_err(|error| error.to_string())?;

    debouncer
        .watcher()
        .watch(&parent, RecursiveMode::NonRecursive)
        .map_err(|error| error.to_string())?;

    Ok(debouncer)
}

fn path_matches_target(event_path: &Path, target: &Path) -> bool {
    matches!(
        (event_path.file_name(), target.file_name()),
        (Some(event_name), Some(target_name)) if event_name == target_name
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_directory() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rightclick-watch-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn target_filter_ignores_sibling_files() {
        assert!(path_matches_target(
            Path::new("/tmp/rightclick/doc.txt"),
            Path::new("/tmp/rightclick/doc.txt")
        ));
        assert!(!path_matches_target(
            Path::new("/tmp/rightclick/other.txt"),
            Path::new("/tmp/rightclick/doc.txt")
        ));
    }

    #[test]
    fn watcher_reports_an_atomic_replacement_of_the_target() {
        let directory = test_directory();
        let target = directory.join("doc.txt");
        let temporary = directory.join("doc.txt.tmp");
        fs::write(&target, b"old").unwrap();
        let (sender, receiver) = mpsc::channel();
        let watcher = watch_with(&target, 7, move |signal| {
            let _ = sender.send(signal);
        })
        .unwrap();

        fs::write(&temporary, b"new").unwrap();
        fs::rename(&temporary, &target).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            WatchSignal::Changed { generation: 7 }
        );

        drop(watcher);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn watcher_reports_target_modification_deletion_and_recreation() {
        let directory = test_directory();
        let target = directory.join("doc.txt");
        fs::write(&target, b"old").unwrap();
        let (sender, receiver) = mpsc::channel();
        let watcher = watch_with(&target, 8, move |signal| {
            let _ = sender.send(signal);
        })
        .unwrap();

        fs::write(&target, b"changed").unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            WatchSignal::Changed { generation: 8 }
        );
        fs::remove_file(&target).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            WatchSignal::Changed { generation: 8 }
        );
        fs::write(&target, b"recreated").unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            WatchSignal::Changed { generation: 8 }
        );

        drop(watcher);
        fs::remove_dir_all(directory).unwrap();
    }
}
