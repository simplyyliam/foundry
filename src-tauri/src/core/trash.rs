//! Moving folders to the OS Recycle Bin, so a delete can be undone from Explorer.

use std::path::Path;

use super::AppError;

/// Moves `path` (a file or folder) to the Recycle Bin. Never deletes permanently without
/// asking: if Windows can't recycle it (e.g. too big for the bin), it shows its own
/// "delete permanently?" prompt, and declining is an error. A failure (say, a locked file)
/// returns `io` with the OS's message and leaves `path` in place.
pub fn move_to_trash(path: &Path) -> Result<(), AppError> {
    if path.parent().is_none() {
        return Err(AppError::invalid_path(format!(
            "refusing to recycle a root folder: {}",
            path.display()
        )));
    }
    imp::recycle(path)?;
    if path.symlink_metadata().is_ok() {
        return Err(io_error(format!(
            "{} was not moved to the Recycle Bin (cancelled)",
            path.display()
        )));
    }
    Ok(())
}

fn io_error(message: String) -> AppError {
    AppError::new(super::ErrorCode::Io, message)
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

    use windows::core::HSTRING;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
        COPYENGINE_E_SHARING_VIOLATION_DEST, COPYENGINE_E_SHARING_VIOLATION_SRC, FOFX_EARLYFAILURE,
        FOFX_RECYCLEONDELETE, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
    };

    use super::{io_error, AppError};

    /// Balances a successful `CoInitializeEx` on this thread.
    struct Com {
        initialized: bool,
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.initialized {
                // SAFETY: paired with the successful CoInitializeEx on this same thread.
                unsafe { CoUninitialize() };
            }
        }
    }

    /// The OS's message plus the HRESULT. Shell copy-engine errors (0x8027…, e.g. a file in
    /// use) have no system message text, so the code is always included.
    fn os_error(path: &Path, err: &windows::core::Error) -> AppError {
        let code = format!("0x{:08X}", err.code().0);
        let in_use = [
            COPYENGINE_E_SHARING_VIOLATION_SRC,
            COPYENGINE_E_SHARING_VIOLATION_DEST,
        ];
        let text = if in_use.contains(&err.code()) {
            "a file in it is open in another program; close it and try again".to_string()
        } else {
            err.message().trim().to_string()
        };
        let detail = if text.is_empty() {
            format!("Windows error {code}")
        } else {
            format!("{text} ({code})")
        };
        io_error(format!(
            "could not move {} to the Recycle Bin: {detail}",
            path.display()
        ))
    }

    pub fn recycle(path: &Path) -> Result<(), AppError> {
        // SAFETY: plain COM calls on this thread with owned arguments. S_FALSE (already
        // initialized) still needs balancing; RPC_E_CHANGED_MODE means another apartment
        // owns the thread, which IFileOperation tolerates, so carry on without balancing.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        let _com = Com {
            initialized: hr.is_ok(),
        };
        let fail = |err: windows::core::Error| os_error(path, &err);
        let target = HSTRING::from(path.as_os_str());

        // SAFETY: every interface is created and used here, on one thread.
        unsafe {
            let op: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(fail)?;
            op.SetOperationFlags(
                FOFX_RECYCLEONDELETE
                    | FOF_NOCONFIRMATION
                    | FOF_NOERRORUI
                    | FOF_SILENT
                    | FOF_WANTNUKEWARNING
                    | FOFX_EARLYFAILURE,
            )
            .map_err(fail)?;
            let item: IShellItem = SHCreateItemFromParsingName(&target, None).map_err(fail)?;
            op.DeleteItem(&item, None).map_err(fail)?;
            op.PerformOperations().map_err(fail)?;
            if op.GetAnyOperationsAborted().map_err(fail)?.as_bool() {
                return Err(io_error(format!(
                    "{} was not moved to the Recycle Bin (cancelled)",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    use super::{io_error, AppError};

    pub fn recycle(path: &Path) -> Result<(), AppError> {
        Err(io_error(format!(
            "moving {} to the trash isn't supported on this OS yet",
            path.display()
        )))
    }
}
