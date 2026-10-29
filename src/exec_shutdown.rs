use std::time::Duration;

use nix::sys::signal;
use nix::unistd;

const GRACE_PERIOD: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Signal(#[from] nix::errno::Errno),

    #[error(transparent)]
    Wait(#[from] std::io::Error),
}

pub async fn terminate(group: unistd::Pid, child: &mut tokio::process::Child) -> Result<(), Error> {
    send_signal(group, signal::Signal::SIGTERM)?;
    let deadline = tokio::time::Instant::now() + GRACE_PERIOD;
    loop {
        child.try_wait()?;
        if !group_exists(group)? {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            send_signal(group, signal::Signal::SIGKILL)?;
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    child.wait().await?;
    Ok(())
}

fn group_exists(group: unistd::Pid) -> Result<bool, Error> {
    let result = signal::killpg(group, None);
    match result {
        Ok(()) => Ok(true),
        Err(error) => {
            if error == nix::errno::Errno::ESRCH {
                Ok(false)
            } else {
                let error = error.into();
                Err(error)
            }
        }
    }
}

fn send_signal(group: unistd::Pid, signal: signal::Signal) -> Result<(), Error> {
    let result = signal::killpg(group, signal);
    match result {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => {
            let error = error.into();
            Err(error)
        }
    }
}
