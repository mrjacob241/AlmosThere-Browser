pub mod ast;
pub mod console;
pub mod effects;
pub mod error;
pub mod html;
pub mod lexer;
pub mod navigator;
pub mod parser;
pub mod screen;
pub mod specs_placeholder;

pub use ast::{Expression, Program, Statement, VarKind};
pub use console::{
    ConsoleLevel, ConsoleMessage, ConsoleSink, INTERPRETER_CONSOLE_INSTRUCTIONS, VecConsole,
    collect_static_console_messages,
};
pub use effects::{
    BrowserEffect, BrowserExecutionState, DomElementSnapshot, DomExecutionState,
    collect_browser_effects,
};
pub use error::{JsError, JsErrorKind};
pub use html::{InlineScript, ScriptParseReport, parse_inline_scripts_from_html};
pub use lexer::{Lexer, Span, Token, TokenKind, lex};
pub use navigator::NavigatorInfo;
pub use parser::{Parser, parse_script};
pub use screen::ScreenInfo;
pub use specs_placeholder::{
    AudioFingerprint, CanvasFingerprint, ErrorShapeInfo, FingerprintSuite, FontList, MathConstants,
    ModernizrFlags, NavigatorPrototypeInfo, OsMediaQueries, OverwriteInfo, StackDepthInfo,
    StorageInfo, TimezoneInfo, TouchInfo, UnknownImageInfo, WebGlInfo,
};
