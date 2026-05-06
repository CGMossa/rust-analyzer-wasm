#![cfg(target_arch = "wasm32")]
#![allow(non_snake_case)]

use std::path::PathBuf;

use base_db::{
    AbsPathBuf, CrateDisplayName, CrateGraphBuilder, CrateName, CrateOrigin, CrateWorkspaceData,
    DependencyBuilder, Env, LangCrateOrigin,
};
use cfg::CfgOptions;
use hir::ChangeWithProcMacros;
use ide::{
    AdjustmentHints, AdjustmentHintsMode, Analysis, AnalysisHost, AssistConfig,
    AssistResolveStrategy, CallableSnippets, ClosureReturnTypeHints, CompletionConfig,
    CompletionFieldsToResolve, DiagnosticsConfig, DiscriminantHints, ExprFillDefaultMode, FileId,
    FilePosition, FileStructureConfig, FindAllRefsConfig, GenericParameterHints,
    GotoDefinitionConfig, GotoImplementationConfig, HighlightConfig, HoverConfig, HoverDocFormat,
    Indel, InlayFieldsToResolve, InlayHintsConfig, InlayKind, LifetimeElisionHints,
    RaFixtureConfig, RenameConfig, SourceRoot, SubstTyLen, TextSize, TypeHintsPlacement,
};
use ide_db::{
    SnippetCap,
    base_db::{FileSet, VfsPath},
    imports::insert_use::{ImportGranularity, InsertUseConfig, PrefixKind},
};
use intern::Symbol;
use triomphe::Arc;
use wasm_bindgen::prelude::*;

mod to_proto;

mod return_types;
use return_types::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    log::info!("worker initialized")
}

#[wasm_bindgen]
pub struct WorldState {
    host: AnalysisHost,
    file_id: FileId,
}

pub fn create_source_root(name: &str, f: FileId) -> SourceRoot {
    let mut file_set = FileSet::default();
    file_set.insert(f, VfsPath::new_virtual_path(format!("/{}/src/lib.rs", name)));
    SourceRoot::new_library(file_set)
}

pub fn create_crate(
    crate_graph: &mut CrateGraphBuilder,
    f: FileId,
    name: &str,
    origin: CrateOrigin,
) -> base_db::CrateBuilderId {
    let mut cfg = CfgOptions::default();
    cfg.insert_atom(Symbol::intern("unix"));
    cfg.insert_key_value(Symbol::intern("target_arch"), Symbol::intern("x86_64"));
    cfg.insert_key_value(Symbol::intern("target_pointer_width"), Symbol::intern("64"));
    let crate_name = CrateName::new(name).unwrap();
    crate_graph.add_crate_root(
        f,
        span::Edition::CURRENT,
        Some(CrateDisplayName::from(crate_name)),
        None,
        cfg,
        None,
        Env::default(),
        origin,
        Vec::new(),
        false,
        Arc::new(AbsPathBuf::assert_utf8(PathBuf::from("/"))),
        Arc::new(CrateWorkspaceData {
            target: Err("wasm demo has no target layout".into()),
            toolchain: None,
        }),
    )
}

pub fn from_single_file(
    text: String,
    fake_std: String,
    fake_core: String,
    fake_alloc: String,
) -> (AnalysisHost, FileId) {
    let mut host = AnalysisHost::default();
    let file_id = FileId::from_raw(0);
    let std_id = FileId::from_raw(1);
    let core_id = FileId::from_raw(2);
    let alloc_id = FileId::from_raw(3);

    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/my_crate/main.rs".to_string()));
    let source_root = SourceRoot::new_local(file_set);

    let mut change = ChangeWithProcMacros::default();
    change.set_roots(vec![
        source_root,
        create_source_root("std", std_id),
        create_source_root("core", core_id),
        create_source_root("alloc", alloc_id),
    ]);
    let mut crate_graph = CrateGraphBuilder::default();
    let my_crate = create_crate(
        &mut crate_graph,
        file_id,
        "my_crate",
        CrateOrigin::Local { repo: None, name: Some(Symbol::intern("my_crate")) },
    );
    let std_crate =
        create_crate(&mut crate_graph, std_id, "std", CrateOrigin::Lang(LangCrateOrigin::Std));
    let core_crate =
        create_crate(&mut crate_graph, core_id, "core", CrateOrigin::Lang(LangCrateOrigin::Core));
    let alloc_crate = create_crate(
        &mut crate_graph,
        alloc_id,
        "alloc",
        CrateOrigin::Lang(LangCrateOrigin::Alloc),
    );
    let core_dep =
        DependencyBuilder::with_prelude(CrateName::new("core").unwrap(), core_crate, true, true);
    let alloc_dep =
        DependencyBuilder::with_prelude(CrateName::new("alloc").unwrap(), alloc_crate, true, true);
    let std_dep =
        DependencyBuilder::with_prelude(CrateName::new("std").unwrap(), std_crate, true, true);

    crate_graph.add_dep(std_crate, core_dep.clone()).unwrap();
    crate_graph.add_dep(std_crate, alloc_dep.clone()).unwrap();
    crate_graph.add_dep(alloc_crate, core_dep.clone()).unwrap();

    crate_graph.add_dep(my_crate, core_dep).unwrap();
    crate_graph.add_dep(my_crate, alloc_dep).unwrap();
    crate_graph.add_dep(my_crate, std_dep).unwrap();

    change.change_file(file_id, Some(text));
    change.change_file(std_id, Some(fake_std));
    change.change_file(core_id, Some(fake_core));
    change.change_file(alloc_id, Some(fake_alloc));
    change.set_crate_graph(crate_graph);
    host.apply_change(change);
    (host, file_id)
}

impl WorldState {
    fn analysis(&self) -> Analysis {
        self.host.analysis()
    }
}

fn ra_fixture() -> RaFixtureConfig<'static> {
    RaFixtureConfig::default()
}

fn insert_use_config() -> InsertUseConfig {
    InsertUseConfig {
        granularity: ImportGranularity::Module,
        enforce_granularity: false,
        prefix_kind: PrefixKind::Plain,
        group: true,
        skip_glob_imports: false,
    }
}

fn completion_config() -> CompletionConfig<'static> {
    CompletionConfig {
        enable_postfix_completions: true,
        enable_imports_on_the_fly: true,
        enable_self_on_the_fly: true,
        enable_auto_iter: true,
        enable_auto_await: true,
        enable_private_editable: true,
        enable_term_search: true,
        term_search_fuel: 1000,
        full_function_signatures: true,
        callable: Some(CallableSnippets::FillArguments),
        add_semicolon_to_unit: true,
        snippet_cap: SnippetCap::new(true),
        insert_use: insert_use_config(),
        prefer_no_std: false,
        prefer_prelude: true,
        prefer_absolute: false,
        snippets: Vec::new(),
        limit: None,
        fields_to_resolve: CompletionFieldsToResolve::empty(),
        exclude_flyimport: Vec::new(),
        exclude_traits: &[],
        ra_fixture: ra_fixture(),
    }
}

fn diagnostics_config() -> DiagnosticsConfig {
    DiagnosticsConfig {
        snippet_cap: SnippetCap::new(true),
        insert_use: insert_use_config(),
        ..DiagnosticsConfig::test_sample()
    }
}

fn hover_config() -> HoverConfig<'static> {
    HoverConfig {
        links_in_hover: true,
        memory_layout: Some(ide::MemoryLayoutHoverConfig {
            size: Some(ide::MemoryLayoutHoverRenderKind::Both),
            offset: Some(ide::MemoryLayoutHoverRenderKind::Both),
            alignment: Some(ide::MemoryLayoutHoverRenderKind::Both),
            niches: true,
            padding: Some(ide::MemoryLayoutHoverRenderKind::Both),
        }),
        documentation: true,
        keywords: true,
        format: HoverDocFormat::Markdown,
        max_trait_assoc_items_count: Some(8),
        max_fields_count: Some(8),
        max_enum_variants_count: Some(8),
        max_subst_ty_len: SubstTyLen::LimitTo(32),
        show_drop_glue: true,
        ra_fixture: ra_fixture(),
    }
}

fn inlay_hints_config() -> InlayHintsConfig<'static> {
    InlayHintsConfig {
        render_colons: true,
        type_hints: true,
        type_hints_placement: TypeHintsPlacement::Inline,
        sized_bound: false,
        discriminant_hints: DiscriminantHints::Fieldless,
        parameter_hints: true,
        parameter_hints_for_missing_arguments: true,
        generic_parameter_hints: GenericParameterHints {
            type_hints: true,
            lifetime_hints: true,
            const_hints: true,
        },
        chaining_hints: true,
        adjustment_hints: AdjustmentHints::BorrowsOnly,
        adjustment_hints_disable_reborrows: false,
        adjustment_hints_mode: AdjustmentHintsMode::Prefix,
        adjustment_hints_hide_outside_unsafe: false,
        closure_return_type_hints: ClosureReturnTypeHints::WithBlock,
        closure_capture_hints: true,
        binding_mode_hints: true,
        implicit_drop_hints: true,
        implied_dyn_trait_hints: true,
        lifetime_elision_hints: LifetimeElisionHints::SkipTrivial,
        param_names_for_lifetime_elision_hints: true,
        hide_inferred_type_hints: false,
        hide_named_constructor_hints: false,
        hide_closure_initialization_hints: false,
        hide_closure_parameter_hints: false,
        range_exclusive_hints: true,
        closure_style: hir::ClosureStyle::RANotation,
        max_length: Some(40),
        closing_brace_hints_min_lines: Some(25),
        fields_to_resolve: InlayFieldsToResolve::empty(),
        ra_fixture: ra_fixture(),
    }
}

fn highlight_config() -> HighlightConfig<'static> {
    HighlightConfig {
        strings: true,
        comments: true,
        punctuation: true,
        specialize_punctuation: true,
        macro_bang: true,
        operator: true,
        specialize_operator: true,
        inject_doc_comment: true,
        syntactic_name_ref_highlighting: false,
        ra_fixture: ra_fixture(),
    }
}

fn goto_definition_config() -> GotoDefinitionConfig<'static> {
    GotoDefinitionConfig { ra_fixture: ra_fixture() }
}

fn goto_implementation_config() -> GotoImplementationConfig {
    GotoImplementationConfig { filter_adjacent_derive_implementations: true }
}

fn find_all_refs_config() -> FindAllRefsConfig<'static> {
    FindAllRefsConfig {
        search_scope: None,
        ra_fixture: ra_fixture(),
        exclude_imports: false,
        exclude_tests: false,
    }
}

fn rename_config() -> RenameConfig {
    RenameConfig {
        prefer_no_std: false,
        prefer_prelude: true,
        prefer_absolute: false,
        show_conflicts: true,
    }
}

fn assist_config() -> AssistConfig {
    AssistConfig {
        snippet_cap: SnippetCap::new(true),
        allowed: None,
        insert_use: insert_use_config(),
        prefer_no_std: false,
        prefer_prelude: true,
        prefer_absolute: false,
        assist_emit_must_use: false,
        term_search_fuel: 1000,
        term_search_borrowck: true,
        code_action_grouping: true,
        expr_fill_default: ExprFillDefaultMode::Todo,
        prefer_self_ty: false,
        show_rename_conflicts: true,
    }
}

#[wasm_bindgen]
impl WorldState {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let (host, file_id) =
            from_single_file("".to_owned(), "".to_owned(), "".to_owned(), "".to_owned());
        Self { host, file_id }
    }

    pub fn init(&mut self, code: String, fake_std: String, fake_core: String, fake_alloc: String) {
        let (host, file_id) = from_single_file(code, fake_std, fake_core, fake_alloc);
        self.host = host;
        self.file_id = file_id;
    }

    pub fn update(&mut self, code: String) -> JsValue {
        log::warn!("update");
        let file_id = FileId::from_raw(0);
        let mut change = ChangeWithProcMacros::default();
        change.change_file(file_id, Some(code));
        self.host.apply_change(change);

        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let highlights: Vec<_> = self
            .analysis()
            .highlight(highlight_config(), file_id)
            .unwrap()
            .into_iter()
            .map(|hl| Highlight {
                tag: Some(hl.highlight.tag.to_string()),
                range: to_proto::text_range(hl.range, &line_index),
            })
            .collect();

        let config = diagnostics_config();

        let diagnostics: Vec<_> = self
            .analysis()
            .full_diagnostics(&config, AssistResolveStrategy::All, file_id)
            .unwrap()
            .into_iter()
            .map(|d| {
                let Range { startLineNumber, startColumn, endLineNumber, endColumn } =
                    to_proto::text_range(d.range.range, &line_index);
                Diagnostic {
                    message: d.message,
                    severity: to_proto::severity(d.severity),
                    startLineNumber,
                    startColumn,
                    endLineNumber,
                    endColumn,
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&UpdateResult { diagnostics, highlights }).unwrap()
    }

    pub fn inlay_hints(&self) -> JsValue {
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let results: Vec<_> = self
            .analysis()
            .inlay_hints(&inlay_hints_config(), self.file_id, None)
            .unwrap()
            .into_iter()
            .map(|ih| InlayHint {
                label: Some(ih.label.to_string()),
                hint_type: match ih.kind {
                    InlayKind::Type | InlayKind::Chaining => InlayHintType::Type,
                    InlayKind::Parameter => InlayHintType::Parameter,
                    _ => InlayHintType::Type,
                },
                range: to_proto::text_range(ih.range, &line_index),
            })
            .collect();
        serde_wasm_bindgen::to_value(&results).unwrap()
    }

    pub fn completions(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("completions");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let res = match self.analysis().completions(&completion_config(), pos, None).unwrap() {
            Some(items) => items,
            None => return JsValue::NULL,
        };

        let items: Vec<_> =
            res.into_iter().map(|item| to_proto::completion_item(item, &line_index)).collect();
        serde_wasm_bindgen::to_value(&items).unwrap()
    }

    pub fn hover(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("hover");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let range = file_range(line_number, column, line_number, column, &line_index, self.file_id);
        let info = match self.analysis().hover(&hover_config(), range).unwrap() {
            Some(info) => info,
            _ => return JsValue::NULL,
        };

        let value = info.info.markup.to_string();
        let hover = Hover {
            contents: vec![MarkdownString { value }],
            range: to_proto::text_range(info.range, &line_index),
        };

        serde_wasm_bindgen::to_value(&hover).unwrap()
    }

    pub fn code_lenses(&self) -> JsValue {
        log::warn!("code_lenses");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let results: Vec<_> = self
            .analysis()
            .file_structure(&FileStructureConfig { exclude_locals: false }, self.file_id)
            .unwrap()
            .into_iter()
            .filter(|it| match it.kind {
                ide::StructureNodeKind::SymbolKind(it) => matches!(
                    it,
                    ide_db::SymbolKind::Trait
                        | ide_db::SymbolKind::Struct
                        | ide_db::SymbolKind::Enum
                ),
                ide::StructureNodeKind::ExternBlock | ide::StructureNodeKind::Region => true,
            })
            .filter_map(|it| {
                let position =
                    FilePosition { file_id: self.file_id, offset: it.node_range.start() };
                let nav_info = self
                    .analysis()
                    .goto_implementation(&goto_implementation_config(), position)
                    .unwrap()?;

                let title = if nav_info.info.len() == 1 {
                    "1 implementation".into()
                } else {
                    format!("{} implementations", nav_info.info.len())
                };

                let positions = nav_info
                    .info
                    .iter()
                    .map(|target| target.focus_range.unwrap_or(target.full_range))
                    .map(|range| to_proto::text_range(range, &line_index))
                    .collect();

                Some(CodeLensSymbol {
                    range: to_proto::text_range(it.node_range, &line_index),
                    command: Some(Command {
                        id: "editor.action.showReferences".into(),
                        title,
                        positions,
                    }),
                })
            })
            .collect();

        serde_wasm_bindgen::to_value(&results).unwrap()
    }

    pub fn references(&self, line_number: u32, column: u32, include_declaration: bool) -> JsValue {
        log::warn!("references");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let ref_results = match self.analysis().find_all_refs(pos, &find_all_refs_config()) {
            Ok(Some(info)) => info,
            _ => return JsValue::NULL,
        };

        let mut res = vec![];
        for ref_result in ref_results {
            if include_declaration
                && let Some(r) = ref_result.declaration
                && r.nav.file_id == self.file_id
            {
                let r = r.nav.focus_range.unwrap_or(r.nav.full_range);
                res.push(Highlight { tag: None, range: to_proto::text_range(r, &line_index) });
            }
            ref_result.references.iter().for_each(|(id, ranges)| {
                if *id != self.file_id {
                    return;
                }
                for (r, _) in ranges {
                    res.push(Highlight { tag: None, range: to_proto::text_range(*r, &line_index) });
                }
            });
        }

        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn prepare_rename(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("prepare_rename");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let range_info = match self.analysis().prepare_rename(pos).unwrap() {
            Ok(refs) => refs,
            _ => return JsValue::NULL,
        };

        let range = to_proto::text_range(range_info.range, &line_index);
        let file_text = self.analysis().file_text(self.file_id).unwrap();
        let text = file_text[range_info.range].to_owned();

        serde_wasm_bindgen::to_value(&RenameLocation { range, text }).unwrap()
    }

    pub fn rename(&self, line_number: u32, column: u32, new_name: &str) -> JsValue {
        log::warn!("rename");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let change = match self.analysis().rename(pos, new_name, &rename_config()).unwrap() {
            Ok(change) => change,
            Err(_) => return JsValue::NULL,
        };

        let result: Vec<_> = change
            .source_file_edits
            .iter()
            .flat_map(|(_, (edit, _))| edit.iter())
            .map(|atom: &Indel| to_proto::text_edit(atom, &line_index))
            .collect();

        serde_wasm_bindgen::to_value(&result).unwrap()
    }

    pub fn signature_help(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("signature_help");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let call_info = match self.analysis().signature_help(pos) {
            Ok(Some(call_info)) => call_info,
            _ => return JsValue::NULL,
        };

        let active_parameter = call_info.active_parameter;
        let sig_info = to_proto::signature_information(call_info);

        let result = SignatureHelp {
            signatures: [sig_info],
            activeSignature: 0,
            activeParameter: active_parameter,
        };
        serde_wasm_bindgen::to_value(&result).unwrap()
    }

    pub fn definition(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("definition");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let nav_info = match self.analysis().goto_definition(pos, &goto_definition_config()) {
            Ok(Some(nav_info)) => nav_info,
            _ => return JsValue::NULL,
        };

        let res = to_proto::location_links(nav_info, &line_index, self.file_id);
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn type_definition(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("type_definition");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let nav_info = match self.analysis().goto_type_definition(pos) {
            Ok(Some(nav_info)) => nav_info,
            _ => return JsValue::NULL,
        };

        let res = to_proto::location_links(nav_info, &line_index, self.file_id);
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn document_symbols(&self) -> JsValue {
        log::warn!("document_symbols");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let struct_nodes = match self
            .analysis()
            .file_structure(&FileStructureConfig { exclude_locals: false }, self.file_id)
        {
            Ok(struct_nodes) => struct_nodes,
            _ => return JsValue::NULL,
        };
        let mut parents: Vec<(DocumentSymbol, Option<usize>)> = Vec::new();

        for symbol in struct_nodes {
            let doc_symbol = DocumentSymbol {
                name: symbol.label.clone(),
                detail: symbol.detail.unwrap_or(symbol.label),
                kind: to_proto::symbol_kind(symbol.kind),
                range: to_proto::text_range(symbol.node_range, &line_index),
                children: None,
                tags: [if symbol.deprecated { SymbolTag::Deprecated } else { SymbolTag::None }],
                containerName: None,
                selectionRange: to_proto::text_range(symbol.navigation_range, &line_index),
            };
            parents.push((doc_symbol, symbol.parent));
        }
        let mut res = Vec::new();
        while let Some((node, parent)) = parents.pop() {
            match parent {
                None => res.push(node),
                Some(i) => {
                    let children = &mut parents[i].0.children;
                    if children.is_none() {
                        *children = Some(Vec::new());
                    }
                    children.as_mut().unwrap().push(node);
                }
            }
        }

        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn type_formatting(&self, line_number: u32, column: u32, ch: char) -> JsValue {
        log::warn!("type_formatting");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let mut pos = file_position(line_number, column, &line_index, self.file_id);
        pos.offset -= TextSize::of('.');

        let edit = self.analysis().on_char_typed(pos, ch);

        let (_file, edit) = match edit {
            Ok(Some(it)) => it.source_file_edits.into_iter().next().unwrap(),
            _ => return JsValue::NULL,
        };

        let change: Vec<TextEdit> = to_proto::text_edits(&edit.0, &line_index);
        serde_wasm_bindgen::to_value(&change).unwrap()
    }

    pub fn folding_ranges(&self) -> JsValue {
        log::warn!("folding_ranges");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        if let Ok(folds) = self.analysis().folding_ranges(self.file_id, false) {
            let res: Vec<_> =
                folds.into_iter().map(|fold| to_proto::folding_range(fold, &line_index)).collect();
            serde_wasm_bindgen::to_value(&res).unwrap()
        } else {
            JsValue::NULL
        }
    }

    pub fn goto_implementation(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("goto_implementation");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();

        let pos = file_position(line_number, column, &line_index, self.file_id);
        let nav_info = match self.analysis().goto_implementation(&goto_implementation_config(), pos)
        {
            Ok(Some(it)) => it,
            _ => return JsValue::NULL,
        };
        let res = to_proto::location_links(nav_info, &line_index, self.file_id);
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn goto_declaration(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("goto_declaration");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let pos = file_position(line_number, column, &line_index, self.file_id);
        let nav_info = match self.analysis().goto_declaration(pos, &goto_definition_config()) {
            Ok(Some(it)) => it,
            _ => return JsValue::NULL,
        };
        let res = to_proto::location_links(nav_info, &line_index, self.file_id);
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn parent_module(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("parent_module");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let pos = file_position(line_number, column, &line_index, self.file_id);
        let navs = match self.analysis().parent_module(pos) {
            Ok(navs) if !navs.is_empty() => navs,
            _ => return JsValue::NULL,
        };
        let res: Vec<_> = navs
            .into_iter()
            .filter(|n| n.file_id == self.file_id)
            .map(|n| to_proto::text_range(n.focus_or_full_range(), &line_index))
            .collect();
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn expand_macro(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("expand_macro");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let pos = file_position(line_number, column, &line_index, self.file_id);
        let expanded = match self.analysis().expand_macro(pos) {
            Ok(Some(it)) => it,
            _ => return JsValue::NULL,
        };
        let result = MacroExpansion { name: expanded.name, expansion: expanded.expansion };
        serde_wasm_bindgen::to_value(&result).unwrap()
    }

    pub fn view_hir(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("view_hir");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let pos = file_position(line_number, column, &line_index, self.file_id);
        match self.analysis().view_hir(pos) {
            Ok(s) => JsValue::from_str(&s),
            Err(_) => JsValue::NULL,
        }
    }

    pub fn view_mir(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("view_mir");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let pos = file_position(line_number, column, &line_index, self.file_id);
        match self.analysis().view_mir(pos) {
            Ok(s) => JsValue::from_str(&s),
            Err(_) => JsValue::NULL,
        }
    }

    pub fn view_syntax_tree(&self) -> JsValue {
        log::warn!("view_syntax_tree");
        match self.analysis().view_syntax_tree(self.file_id) {
            Ok(s) => JsValue::from_str(&s),
            Err(_) => JsValue::NULL,
        }
    }

    pub fn view_item_tree(&self) -> JsValue {
        log::warn!("view_item_tree");
        match self.analysis().view_item_tree(self.file_id) {
            Ok(s) => JsValue::from_str(&s),
            Err(_) => JsValue::NULL,
        }
    }

    pub fn assists(
        &self,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> JsValue {
        log::warn!("assists");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let frange =
            file_range(start_line, start_column, end_line, end_column, &line_index, self.file_id);
        let assists = match self.analysis().assists_with_fixes(
            &assist_config(),
            &diagnostics_config(),
            AssistResolveStrategy::All,
            frange,
        ) {
            Ok(items) => items,
            _ => return JsValue::NULL,
        };
        let res: Vec<_> = assists
            .into_iter()
            .map(|a| {
                let edits: Vec<TextEdit> = a
                    .source_change
                    .as_ref()
                    .and_then(|sc| {
                        sc.source_file_edits
                            .iter()
                            .find(|(fid, _)| **fid == self.file_id)
                            .map(|(_, (te, _))| to_proto::text_edits(te, &line_index))
                    })
                    .unwrap_or_default();
                Assist {
                    label: a.label.to_string(),
                    group: a.group.map(|g| g.0),
                    kind: a.id.1.name().to_owned(),
                    target: to_proto::text_range(a.target, &line_index),
                    edits,
                }
            })
            .collect();
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn runnables(&self) -> JsValue {
        log::warn!("runnables");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let runnables = match self.analysis().runnables(self.file_id) {
            Ok(rs) => rs,
            _ => return JsValue::NULL,
        };
        let res: Vec<_> = runnables
            .into_iter()
            .map(|r| {
                let label = match &r.kind {
                    ide::RunnableKind::Test { test_id, .. } => format!("test {}", test_id),
                    ide::RunnableKind::TestMod { path } => format!("test mod {}", path),
                    ide::RunnableKind::Bench { test_id } => format!("bench {}", test_id),
                    ide::RunnableKind::DocTest { test_id } => format!("doctest {}", test_id),
                    ide::RunnableKind::Bin => "run".to_string(),
                };
                Runnable {
                    label,
                    range: to_proto::text_range(r.nav.focus_or_full_range(), &line_index),
                }
            })
            .collect();
        serde_wasm_bindgen::to_value(&res).unwrap()
    }

    pub fn join_lines(
        &self,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> JsValue {
        log::warn!("join_lines");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let frange =
            file_range(start_line, start_column, end_line, end_column, &line_index, self.file_id);
        let edit = self
            .analysis()
            .join_lines(
                &ide::JoinLinesConfig {
                    join_else_if: true,
                    remove_trailing_comma: true,
                    unwrap_trivial_blocks: true,
                    join_assignments: true,
                },
                frange,
            )
            .unwrap_or_default();
        let edits = to_proto::text_edits(&edit, &line_index);
        serde_wasm_bindgen::to_value(&edits).unwrap()
    }

    pub fn matching_brace(&self, line_number: u32, column: u32) -> JsValue {
        log::warn!("matching_brace");
        let line_index = self.analysis().file_line_index(self.file_id).unwrap();
        let pos = file_position(line_number, column, &line_index, self.file_id);
        match self.analysis().matching_brace(pos) {
            Ok(Some(offset)) => {
                let pos = line_index.line_col(offset);
                serde_wasm_bindgen::to_value(&BracePosition {
                    lineNumber: pos.line + 1,
                    column: pos.col + 1,
                })
                .unwrap()
            }
            _ => JsValue::NULL,
        }
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

fn file_position(
    line_number: u32,
    column: u32,
    line_index: &ide::LineIndex,
    file_id: ide::FileId,
) -> ide::FilePosition {
    let line_col = ide::LineCol { line: line_number - 1, col: column - 1 };
    let offset = line_index.offset(line_col).unwrap_or(TextSize::from(0));
    ide::FilePosition { file_id, offset }
}

fn file_range(
    start_line_number: u32,
    start_column: u32,
    end_line_number: u32,
    end_column: u32,
    line_index: &ide::LineIndex,
    file_id: ide::FileId,
) -> ide::FileRange {
    let start_line_col = ide::LineCol { line: start_line_number - 1, col: start_column - 1 };
    let end_line_col = ide::LineCol { line: end_line_number - 1, col: end_column - 1 };
    ide::FileRange {
        file_id,
        range: ide::TextRange::new(
            line_index.offset(start_line_col).unwrap_or(TextSize::from(0)),
            line_index.offset(end_line_col).unwrap_or(TextSize::from(0)),
        ),
    }
}
