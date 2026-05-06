//! Conversion of rust-analyzer specific types to return_types equivalents.

use crate::return_types;

pub(crate) fn text_range(
    range: ide::TextRange,
    line_index: &ide::LineIndex,
) -> return_types::Range {
    let start = line_index.line_col(range.start());
    let end = line_index.line_col(range.end());

    return_types::Range {
        startLineNumber: start.line + 1,
        startColumn: start.col + 1,
        endLineNumber: end.line + 1,
        endColumn: end.col + 1,
    }
}

pub(crate) fn completion_item_kind(
    kind: ide::CompletionItemKind,
) -> return_types::CompletionItemKind {
    use return_types::CompletionItemKind::*;
    match kind {
        ide::CompletionItemKind::Keyword => Keyword,
        ide::CompletionItemKind::Snippet => Snippet,
        ide::CompletionItemKind::BuiltinType | ide::CompletionItemKind::InferredType => Struct,
        ide::CompletionItemKind::Binding => Variable,
        ide::CompletionItemKind::Expression => Value,
        ide::CompletionItemKind::UnresolvedReference => User,
        ide::CompletionItemKind::SymbolKind(it) => symbol_completion_kind(it),
    }
}

fn symbol_completion_kind(kind: ide::SymbolKind) -> return_types::CompletionItemKind {
    use return_types::CompletionItemKind::*;
    match kind {
        ide::SymbolKind::Const | ide::SymbolKind::ConstParam => Constant,
        ide::SymbolKind::Enum => Enum,
        ide::SymbolKind::Field => Field,
        ide::SymbolKind::Function | ide::SymbolKind::Macro | ide::SymbolKind::ProcMacro => Function,
        ide::SymbolKind::Impl | ide::SymbolKind::Trait => Interface,
        ide::SymbolKind::Label => Constant,
        ide::SymbolKind::LifetimeParam
        | ide::SymbolKind::SelfType
        | ide::SymbolKind::TypeAlias
        | ide::SymbolKind::TypeParam => TypeParameter,
        ide::SymbolKind::Local | ide::SymbolKind::SelfParam => Variable,
        ide::SymbolKind::Method => Method,
        ide::SymbolKind::Module | ide::SymbolKind::CrateRoot => Module,
        ide::SymbolKind::Static => Value,
        ide::SymbolKind::Struct | ide::SymbolKind::Union => Struct,
        ide::SymbolKind::ValueParam => TypeParameter,
        ide::SymbolKind::Variant => EnumMember,
        ide::SymbolKind::Attribute
        | ide::SymbolKind::BuiltinAttr
        | ide::SymbolKind::Derive
        | ide::SymbolKind::DeriveHelper => Property,
        ide::SymbolKind::InlineAsmRegOrRegClass | ide::SymbolKind::ToolModule => User,
    }
}

pub(crate) fn severity(s: ide::Severity) -> return_types::MarkerSeverity {
    match s {
        ide::Severity::Error => return_types::MarkerSeverity::Error,
        ide::Severity::Warning => return_types::MarkerSeverity::Warning,
        ide::Severity::WeakWarning => return_types::MarkerSeverity::Hint,
        ide::Severity::Allow => return_types::MarkerSeverity::Info,
    }
}

pub(crate) fn text_edit(indel: &ide::Indel, line_index: &ide::LineIndex) -> return_types::TextEdit {
    let text = indel.insert.clone();
    return_types::TextEdit { range: text_range(indel.delete, line_index), text }
}

pub(crate) fn text_edits(
    edit: &ide::TextEdit,
    ctx: &ide::LineIndex,
) -> Vec<return_types::TextEdit> {
    edit.iter().map(|atom| text_edit(atom, ctx)).collect()
}

pub(crate) fn completion_item(
    item: ide::CompletionItem,
    line_index: &ide::LineIndex,
) -> return_types::CompletionItem {
    let mut text_edits = item.text_edit.iter();
    let edit = text_edits.next().map(|it| text_edit(it, line_index)).unwrap_or_else(|| {
        return_types::TextEdit {
            range: text_range(item.source_range, line_index),
            text: item.label.primary.to_string(),
        }
    });
    let additional_text_edits = text_edits.map(|it| text_edit(it, line_index)).collect();
    let return_types::TextEdit { range, text } = edit;

    return_types::CompletionItem {
        kind: completion_item_kind(item.kind),
        label: item.label.primary.to_string(),
        range,
        detail: item.detail,
        insertText: text,
        insertTextRules: if item.is_snippet {
            return_types::CompletionItemInsertTextRule::InsertAsSnippet
        } else {
            return_types::CompletionItemInsertTextRule::None
        },
        documentation: item.documentation.map(|doc| markdown_string(doc.as_str())),
        filterText: item.lookup.to_string(),
        additionalTextEdits: additional_text_edits,
    }
}

pub(crate) fn signature_information(
    call_info: ide::SignatureHelp,
) -> return_types::SignatureInformation {
    use return_types::{ParameterInformation, SignatureInformation};

    let label = call_info.signature.clone();
    let documentation = call_info.doc.as_ref().map(|it| markdown_string(it.as_str()));

    let parameters: Vec<ParameterInformation> = call_info
        .parameter_labels()
        .map(|param| ParameterInformation { label: param.to_string() })
        .collect();

    SignatureInformation { label, documentation, parameters }
}

pub(crate) fn location_links(
    nav_info: ide::RangeInfo<Vec<ide::NavigationTarget>>,
    line_index: &ide::LineIndex,
    file_id: ide::FileId,
) -> Vec<return_types::LocationLink> {
    let selection = text_range(nav_info.range, line_index);
    nav_info
        .info
        .into_iter()
        .filter(|nav| nav.file_id == file_id)
        .map(|nav| {
            let range = text_range(nav.full_range, line_index);
            let target_selection_range =
                nav.focus_range.map(|it| text_range(it, line_index)).unwrap_or(range);

            return_types::LocationLink {
                originSelectionRange: selection,
                range,
                targetSelectionRange: target_selection_range,
            }
        })
        .collect()
}

pub(crate) fn symbol_kind(kind: ide::StructureNodeKind) -> return_types::SymbolKind {
    use return_types::SymbolKind;

    let kind = match kind {
        ide::StructureNodeKind::SymbolKind(it) => it,
        ide::StructureNodeKind::ExternBlock | ide::StructureNodeKind::Region => {
            return SymbolKind::Property;
        }
    };

    match kind {
        ide::SymbolKind::Const | ide::SymbolKind::ConstParam | ide::SymbolKind::Static => {
            SymbolKind::Constant
        }
        ide::SymbolKind::Enum => SymbolKind::Enum,
        ide::SymbolKind::Field => SymbolKind::Field,
        ide::SymbolKind::Function | ide::SymbolKind::Macro | ide::SymbolKind::ProcMacro => {
            SymbolKind::Function
        }
        ide::SymbolKind::Impl | ide::SymbolKind::Trait => SymbolKind::Interface,
        ide::SymbolKind::Label => SymbolKind::Constant,
        ide::SymbolKind::LifetimeParam
        | ide::SymbolKind::SelfType
        | ide::SymbolKind::TypeAlias
        | ide::SymbolKind::TypeParam
        | ide::SymbolKind::ValueParam => SymbolKind::TypeParameter,
        ide::SymbolKind::Local | ide::SymbolKind::SelfParam => SymbolKind::Variable,
        ide::SymbolKind::Method => SymbolKind::Method,
        ide::SymbolKind::Module | ide::SymbolKind::CrateRoot => SymbolKind::Module,
        ide::SymbolKind::Struct | ide::SymbolKind::Union => SymbolKind::Struct,
        ide::SymbolKind::Variant => SymbolKind::EnumMember,
        ide::SymbolKind::Attribute
        | ide::SymbolKind::BuiltinAttr
        | ide::SymbolKind::Derive
        | ide::SymbolKind::DeriveHelper => SymbolKind::Property,
        ide::SymbolKind::InlineAsmRegOrRegClass | ide::SymbolKind::ToolModule => SymbolKind::Object,
    }
}

pub(crate) fn folding_range(fold: ide::Fold, ctx: &ide::LineIndex) -> return_types::FoldingRange {
    let range = text_range(fold.range, ctx);
    return_types::FoldingRange {
        start: range.startLineNumber,
        end: range.endLineNumber,
        kind: match fold.kind {
            ide::FoldKind::Comment => Some(return_types::FoldingRangeKind::Comment),
            ide::FoldKind::Imports => Some(return_types::FoldingRangeKind::Imports),
            ide::FoldKind::Region => Some(return_types::FoldingRangeKind::Region),
            _ => None,
        },
    }
}

fn markdown_string(s: &str) -> return_types::MarkdownString {
    fn code_line_ignored_by_rustdoc(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed == "#" || trimmed.starts_with("# ") || trimmed.starts_with("#\t")
    }

    let mut processed_lines = Vec::new();
    let mut in_code_block = false;
    for line in s.lines() {
        if in_code_block && code_line_ignored_by_rustdoc(line) {
            continue;
        }

        if line.starts_with("```") {
            in_code_block ^= true
        }

        let line = if in_code_block && line.starts_with("```") && !line.contains("rust") {
            "```rust"
        } else {
            line
        };

        processed_lines.push(line);
    }

    return_types::MarkdownString { value: processed_lines.join("\n") }
}
