use std::thread;

use reedline::{
    ColumnarMenu, Completer, DefaultPrompt, DefaultPromptSegment, Emacs,
    EditCommand, ExternalPrinter, KeyCode, KeyModifiers, MenuBuilder, Reedline, ReedlineEvent,
    ReedlineMenu, Signal, Span, Suggestion, default_emacs_keybindings,
};
use tokio::sync::mpsc;

const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/to",      "<client>  切换目标用户（留空恢复广播）"),
    ("/group",   "<group>   切换目标分组（留空恢复默认）"),
    ("/json",    "<json>    发送 JSON 消息"),
    ("/command", "<cmd>     发送命令请求（仅单播）"),
    ("/file",    "<path>    发送文件（仅单播）"),
    ("/files",   "          查看本地文件状态"),
    ("/accept",  "<fileId>  接收待确认文件"),
    ("/reject",  "<fileId>  拒绝待确认文件"),
    ("/abort",   "<fileId>  取消文件传输"),
    ("/status",  "          查看当前会话"),
    ("/help",    "          查看帮助"),
    ("/exit",    "          退出会话"),
];

struct SlashCompleter;

impl Completer for SlashCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let prefix = &line[..pos];
        // Only complete first token if it starts with '/'
        if !prefix.starts_with('/') || prefix.contains(' ') {
            return vec![];
        }
        SLASH_COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(prefix))
            .map(|(cmd, desc)| Suggestion {
                value: cmd.to_string(),
                description: Some(desc.to_string()),
                style: None,
                extra: None,
                span: Span::new(0, pos),
                append_whitespace: true,
                display_override: None,
                match_indices: None,
            })
            .collect()
    }
}

/// Spawn the reedline editor thread. Returns an `ExternalPrinter` handle that
/// other threads (tokio tasks, plugins) can use to print above the active prompt.
pub(crate) fn spawn_line_editor(line_tx: mpsc::UnboundedSender<String>) -> ExternalPrinter<String> {
    let printer = ExternalPrinter::default();
    let printer_for_editor = printer.clone();

    thread::spawn(move || {
        let completer = Box::new(SlashCompleter);
        let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));

        // Tab opens the completion menu; typing '/' inserts the char and opens menu.
        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Char('/'),
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Edit(vec![EditCommand::InsertChar('/')]),
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );

        let mut editor = Reedline::create()
            .with_completer(completer)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(Box::new(Emacs::new(keybindings)))
            .with_external_printer(printer_for_editor);

        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic("y2m> ".to_string()),
            DefaultPromptSegment::Empty,
        );

        loop {
            match editor.read_line(&prompt) {
                Ok(Signal::Success(line)) => {
                    if line_tx.send(line).is_err() {
                        break;
                    }
                }
                // CtrlC clears the current line — stay in loop.
                Ok(Signal::CtrlC) => continue,
                // CtrlD / error — EOF, treat same as /exit.
                Ok(Signal::CtrlD) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    });

    printer
}
