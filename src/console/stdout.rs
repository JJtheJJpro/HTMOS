pub fn clear() -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.clear())
}
/*
pub fn current_mode() -> Result<Option<uefi::proto::console::text::OutputMode>, uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.current_mode())
}
pub fn cursor_position() -> (usize, usize) {
    uefi::system::with_stdout(|stdout| stdout.cursor_position())
}
pub fn cursor_visible() -> bool {
    uefi::system::with_stdout(|stdout| stdout.cursor_visible())
}
pub fn enable_cursor(visible: bool) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.enable_cursor(visible))
}
pub fn modes() -> alloc::vec::Vec<uefi::proto::console::text::OutputMode> {
    uefi::system::with_stdout(|stdout| {
        stdout
            .modes()
            .collect::<alloc::vec::Vec<uefi::proto::console::text::OutputMode>>()
    })
}
pub fn output_string(string: &uefi::CStr16) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.output_string(string))
}
pub fn output_string_lossy(string: &uefi::CStr16) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.output_string_lossy(string))
}
pub fn reset(extended: bool) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.reset(extended))
}
pub fn set_color(
    foreground: uefi::proto::console::text::Color,
    background: uefi::proto::console::text::Color,
) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.set_color(foreground, background))
}
pub fn set_cursor_position(column: usize, row: usize) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.set_cursor_position(column, row))
}
pub fn set_mode(mode: uefi::proto::console::text::OutputMode) -> Result<(), uefi::Error> {
    uefi::system::with_stdout(|stdout| stdout.set_mode(mode))
}
*/