//! Firefox native-messaging to Deepify desktop socket proxy.
//!
//! This binary deliberately has no session, pairing, whitelist, or URL policy.
//! It only accepts bounded protocol frames and copies them in both directions.
use deepify_browser_protocol::{parse_frame, MAX_FRAME_BYTES};
use std::{
    env, fs,
    io::{self, Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    thread,
};

const SOCKET_RELATIVE_PATH: &str = "deepify/browser-v1.sock";

fn socket_path() -> io::Result<PathBuf> {
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "XDG_RUNTIME_DIR is required"))?;
    checked_socket_path(&runtime)
}

fn checked_socket_path(runtime: &Path) -> io::Result<PathBuf> {
    let metadata = fs::symlink_metadata(runtime)?;
    if !secure_runtime_directory(&metadata, unsafe { libc::geteuid() }) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "insecure runtime directory",
        ));
    }
    let path = runtime.join(SOCKET_RELATIVE_PATH);
    let directory = fs::symlink_metadata(path.parent().unwrap())?;
    let socket = fs::symlink_metadata(&path)?;
    if !secure_runtime_directory(&directory, unsafe { libc::geteuid() })
        || !socket.file_type().is_socket()
        || socket.uid() != unsafe { libc::geteuid() }
        || socket.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "insecure browser socket",
        ));
    }
    Ok(path)
}

fn secure_runtime_directory(metadata: &fs::Metadata, expected_uid: u32) -> bool {
    metadata.is_dir()
        && metadata.uid() == expected_uid
        && metadata.permissions().mode() & 0o077 == 0
}

fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0; 4];
    if reader.read(&mut length[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut length[1..])?;
    let size = u32::from_le_bytes(length) as usize;
    if size > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized browser frame",
        ));
    }
    let mut body = vec![0; size];
    reader.read_exact(&mut body)?;
    parse_frame(&body)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid browser frame"))?;
    Ok(Some(body))
}

fn write_frame(writer: &mut impl Write, body: &[u8]) -> io::Result<()> {
    if body.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized browser frame",
        ));
    }
    writer.write_all(&(body.len() as u32).to_le_bytes())?;
    writer.write_all(body)?;
    writer.flush()
}

fn copy_stdio_to_socket(mut socket: UnixStream) {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    while let Ok(Some(frame)) = read_frame(&mut input) {
        if write_frame(&mut socket, &frame).is_err() {
            break;
        }
    }
    let _ = socket.shutdown(std::net::Shutdown::Both);
}

fn proxy(socket: UnixStream) -> io::Result<()> {
    let input_socket = socket.try_clone()?;
    let _reader = thread::spawn(move || copy_stdio_to_socket(input_socket));
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut socket = socket;
    while let Some(frame) = read_frame(&mut socket)? {
        write_frame(&mut output, &frame)?;
    }
    let _ = socket.shutdown(std::net::Shutdown::Both);
    // Do not join the stdin-forwarding thread here. Firefox keeps native-host
    // stdin open until this process exits; joining would strand the helper
    // after a desktop socket close and prevent the extension's fail-open path.
    Ok(())
}

fn main() -> io::Result<()> {
    let path = socket_path()?;
    let socket = UnixStream::connect(path)?;
    proxy(socket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const HEARTBEAT: &[u8] = br#"{"version":1,"type":"heartbeat","message_id":"test-1"}"#;

    #[test]
    fn frames_handle_fragmented_in_memory_data() {
        let mut encoded = Vec::new();
        write_frame(&mut encoded, HEARTBEAT).unwrap();
        let mut cursor = Cursor::new(encoded);
        assert_eq!(read_frame(&mut cursor).unwrap().unwrap(), HEARTBEAT);
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn oversized_and_invalid_frames_are_rejected() {
        let mut oversized = (MAX_FRAME_BYTES as u32 + 1).to_le_bytes().to_vec();
        oversized.extend_from_slice(b"ignored");
        assert!(read_frame(&mut Cursor::new(oversized)).is_err());
        let mut invalid = Vec::new();
        write_frame(
            &mut invalid,
            br#"{"version":1,"type":"unknown","message_id":"x"}"#,
        )
        .unwrap();
        assert!(read_frame(&mut Cursor::new(invalid)).is_err());
    }

    #[test]
    fn runtime_socket_requires_private_directory() {
        let temp = tempfile::tempdir().unwrap();
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let metadata = fs::metadata(temp.path()).unwrap();
        assert!(secure_runtime_directory(&metadata, metadata.uid()));
        assert!(!secure_runtime_directory(
            &metadata,
            metadata.uid().saturating_add(1)
        ));
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!secure_runtime_directory(
            &fs::metadata(temp.path()).unwrap(),
            metadata.uid()
        ));
    }

    #[test]
    fn host_rejects_redirected_or_public_socket_paths() {
        use std::os::unix::net::UnixListener;
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let directory = root.path().join("deepify");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = directory.join("browser-v1.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(checked_socket_path(root.path()).is_ok());
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(checked_socket_path(root.path()).is_err());
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(root.path(), &alias).unwrap();
        assert!(checked_socket_path(&alias).is_err());
    }
}
