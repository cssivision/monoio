use std::{error::Error, fmt, io, rc::Rc};

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{AsyncReadRent, AsyncWriteRent},
};

use super::{SocketAddr, UnixStream};

/// ReadHalf.
#[derive(Debug)]
pub struct ReadHalf<'a>(&'a UnixStream);

/// WriteHalf.
#[derive(Debug)]
pub struct WriteHalf<'a>(&'a UnixStream);

pub(crate) fn split(stream: &mut UnixStream) -> (ReadHalf<'_>, WriteHalf<'_>) {
    (ReadHalf(&*stream), WriteHalf(&*stream))
}

impl<'t> AsyncReadRent for ReadHalf<'t> {
    type ReadFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ReadvFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;

    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        self.0.read(buf)
    }

    fn readv<T: IoVecBufMut>(&self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        self.0.readv(buf)
    }
}

impl<'t> AsyncWriteRent for WriteHalf<'t> {
    type WriteFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type WritevFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ShutdownFuture<'a>
    where
        't: 'a,
    = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        self.0.write(buf)
    }

    fn writev<T: IoVecBuf>(&self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        self.0.writev(buf_vec)
    }

    fn shutdown(&self) -> Self::ShutdownFuture<'_> {
        self.0.shutdown()
    }
}

/// OwnedReadHalf.
#[derive(Debug)]
pub struct OwnedReadHalf(Rc<UnixStream>);

/// OwnedWriteHalf.
#[derive(Debug)]
pub struct OwnedWriteHalf(Rc<UnixStream>);

pub(crate) fn split_owned(stream: UnixStream) -> (OwnedReadHalf, OwnedWriteHalf) {
    let stream_shared = Rc::new(stream);
    (
        OwnedReadHalf(stream_shared.clone()),
        OwnedWriteHalf(stream_shared),
    )
}

pub(crate) fn reunite(
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
) -> Result<UnixStream, ReuniteError> {
    if Rc::ptr_eq(&read.0, &write.0) {
        drop(write);
        // This unwrap cannot fail as the api does not allow creating more than two Arcs,
        // and we just dropped the other half.
        Ok(Rc::try_unwrap(read.0).expect("UnixStream: try_unwrap failed in reunite"))
    } else {
        Err(ReuniteError(read, write))
    }
}

/// Error indicating that two halves were not from the same socket, and thus could
/// not be reunited.
#[derive(Debug)]
pub struct ReuniteError(pub OwnedReadHalf, pub OwnedWriteHalf);

impl fmt::Display for ReuniteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tried to reunite halves that are not from the same socket"
        )
    }
}

impl Error for ReuniteError {}

impl OwnedReadHalf {
    /// Attempts to put the two halves of a `TcpStream` back together and
    /// recover the original socket. Succeeds only if the two halves
    /// originated from the same call to [`into_split`].
    ///
    /// [`into_split`]: TcpStream::into_split()
    pub fn reunite(self, other: OwnedWriteHalf) -> Result<UnixStream, ReuniteError> {
        reunite(self, other)
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

impl AsyncReadRent for OwnedReadHalf {
    type ReadFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ReadvFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;

    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        self.0.read(buf)
    }

    fn readv<T: IoVecBufMut>(&self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        self.0.readv(buf)
    }
}

impl AsyncWriteRent for OwnedWriteHalf {
    type WriteFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type WritevFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ShutdownFuture<'a> = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        self.0.write(buf)
    }

    fn writev<T: IoVecBuf>(&self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        self.0.writev(buf_vec)
    }

    fn shutdown(&self) -> Self::ShutdownFuture<'_> {
        self.0.shutdown()
    }
}
