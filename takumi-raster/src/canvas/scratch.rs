/// Uninitialized scratch buffer for callers that overwrite every byte before
/// reading; skips the memset a zeroed allocation would pay.
#[allow(clippy::uninit_vec)]
pub(crate) fn uninit_buffer(len: usize) -> Vec<u8> {
  let mut buf = Vec::with_capacity(len);

  unsafe {
    buf.set_len(len);
  }
  buf
}
