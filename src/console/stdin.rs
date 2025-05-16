pub fn read_key() -> Result<Option<uefi::proto::console::text::Key>, uefi::Error> {
    uefi::system::with_stdin(|stdin| stdin.read_key())
}
//pub fn reset(extended_verification: bool) -> Result<(), uefi::Error> {
//    uefi::system::with_stdin(|stdin| stdin.reset(extended_verification))
//}
pub fn wait_for_key_event() -> Option<uefi::Event> {
    uefi::system::with_stdin(|stdin| stdin.wait_for_key_event())
}
