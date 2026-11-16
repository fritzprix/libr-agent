//! TUI `/knowledge` list, search, entity inspect, and chunk delete (#2818).
//!
//! Hosting process only: reads SQLite via `KnowledgeV2Repository` (same path as
//! the GUI Tauri commands). Attached TUI without an in-process repo is refused.

use crate::mcp::builtin::knowledge::helpers::parse_db_tags;
use crate::repositories::{
    KnowledgeChunkDetail, KnowledgeGraphEntity, KnowledgeListCursor, KnowledgeV2Repository,
};
use crate::state::try_get_knowledge_v2_repository;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

const PAGE_SIZE: u64 = 60;
const PREVIEW_LEN: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeCommand {
    /// Open or refresh the chunk list (optional search / assistant filter).
    Open {
        query: Option<String>,
        assistant_id: Option<String>,
    },
    /// Open entity detail (S6) and optionally filter the list by entity name.
    Entity { id: i32 },
}

/// Parse `/knowledge …` composer lines (parity §3.3 / #2818).
pub fn parse_knowledge_command(text: &str) -> Option<KnowledgeCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let rest = trimmed[1..].trim_start();
    let mut tokens = rest.split_whitespace();
    if !tokens.next()?.eq_ignore_ascii_case("knowledge") {
        return None;
    }

    let mut query: Option<String> = None;
    let mut assistant_id: Option<String> = None;
    let mut entity_id: Option<i32> = None;
    let mut search_words: Vec<String> = Vec::new();
    let mut expecting_assistant = false;
    let mut expecting_entity = false;
    let mut in_search = false;

    for tok in tokens {
        if expecting_assistant {
            if tok != "all" {
                assistant_id = Some(tok.to_string());
            }
            expecting_assistant = false;
            continue;
        }
        if expecting_entity {
            entity_id = Some(tok.parse().ok()?);
            expecting_entity = false;
            continue;
        }
        match tok {
            "--assistant" => expecting_assistant = true,
            "entity" if !in_search && search_words.is_empty() && entity_id.is_none() => {
                expecting_entity = true;
            }
            "search" if !in_search && search_words.is_empty() => in_search = true,
            t if t.starts_with("--") => return None,
            t => search_words.push(t.to_string()),
        }
    }

    if expecting_assistant || expecting_entity {
        return None;
    }
    if !search_words.is_empty() {
        query = Some(search_words.join(" "));
    }
    if let Some(id) = entity_id {
        return Some(KnowledgeCommand::Entity { id });
    }
    Some(KnowledgeCommand::Open {
        query,
        assistant_id,
    })
}

#[derive(Debug, Clone)]
pub struct ChunkRow {
    pub id: i32,
    pub assistant_id: String,
    pub preview: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
enum View {
    List,
    ChunkDetail {
        detail: KnowledgeChunkDetail,
        scroll: u16,
    },
    EntityDetail {
        entity: KnowledgeGraphEntity,
        related: Vec<String>,
        scroll: u16,
    },
    ConfirmDelete {
        chunk_id: i32,
    },
}

#[derive(Debug)]
pub struct KnowledgeScreen {
    query: Option<String>,
    assistant_id: Option<String>,
    items: Vec<ChunkRow>,
    next_cursor: Option<KnowledgeListCursor>,
    selected: usize,
    view: View,
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeAction {
    None,
    Close,
    Reload,
    LoadMore,
    OpenChunk(i32),
    ConfirmDelete(i32),
    Delete(i32),
    CancelDelete,
}

impl KnowledgeScreen {
    pub async fn open(query: Option<String>, assistant_id: Option<String>) -> Result<Self, String> {
        let mut screen = Self {
            query,
            assistant_id,
            items: Vec::new(),
            next_cursor: None,
            selected: 0,
            view: View::List,
            notice: None,
        };
        screen.reload().await?;
        Ok(screen)
    }

    pub async fn open_entity(id: i32) -> Result<Self, String> {
        let repo = try_get_knowledge_v2_repository().ok_or_else(|| {
            "Knowledge requires the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let graph = repo
            .get_global_graph(None, 500)
            .await
            .map_err(|e| e.to_string())?;
        let entity = graph
            .entities
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| format!("Entity {id} not found"))?;
        let related = graph
            .relationships
            .iter()
            .filter(|r| r.source_entity_id == id || r.target_entity_id == id)
            .map(|r| {
                let other = if r.source_entity_id == id {
                    r.target_entity_id
                } else {
                    r.source_entity_id
                };
                let other_name = graph
                    .entities
                    .iter()
                    .find(|e| e.id == other)
                    .map(|e| e.name.as_str())
                    .unwrap_or("?");
                format!("{} → {other_name} (#{other})", r.relation_type)
            })
            .collect::<Vec<_>>();

        let mut screen =
            Self::open(Some(entity.name.clone()), Some(entity.assistant_id.clone())).await?;
        screen.view = View::EntityDetail {
            entity,
            related,
            scroll: 0,
        };
        Ok(screen)
    }

    pub async fn reload(&mut self) -> Result<(), String> {
        let repo = try_get_knowledge_v2_repository().ok_or_else(|| {
            "Knowledge requires the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let page = repo
            .list_chunks(
                self.assistant_id.as_deref(),
                self.query.as_deref(),
                None,
                PAGE_SIZE,
            )
            .await
            .map_err(|e| e.to_string())?;
        self.items = page
            .items
            .into_iter()
            .map(|chunk| ChunkRow {
                id: chunk.id,
                assistant_id: chunk.assistant_id,
                preview: build_preview(&chunk.content),
                tags: parse_db_tags(chunk.tags.as_ref()),
            })
            .collect();
        self.next_cursor = page.next_cursor;
        self.selected = 0;
        self.view = View::List;
        Ok(())
    }

    pub async fn load_more(&mut self) -> Result<(), String> {
        let Some(cursor) = self.next_cursor else {
            self.notice = Some("No more chunks".into());
            return Ok(());
        };
        let repo = try_get_knowledge_v2_repository().ok_or_else(|| {
            "Knowledge requires the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let page = repo
            .list_chunks(
                self.assistant_id.as_deref(),
                self.query.as_deref(),
                Some(cursor),
                PAGE_SIZE,
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut seen: std::collections::HashSet<i32> = self.items.iter().map(|r| r.id).collect();
        for chunk in page.items {
            if seen.insert(chunk.id) {
                self.items.push(ChunkRow {
                    id: chunk.id,
                    assistant_id: chunk.assistant_id,
                    preview: build_preview(&chunk.content),
                    tags: parse_db_tags(chunk.tags.as_ref()),
                });
            }
        }
        self.next_cursor = page.next_cursor;
        Ok(())
    }

    pub async fn open_chunk(&mut self, id: i32) -> Result<(), String> {
        let repo = try_get_knowledge_v2_repository().ok_or_else(|| {
            "Knowledge requires the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let detail = repo.get_chunk_detail(id).await.map_err(|e| e.to_string())?;
        self.view = View::ChunkDetail { detail, scroll: 0 };
        Ok(())
    }

    pub async fn delete_chunk(&mut self, id: i32) -> Result<(), String> {
        let repo = try_get_knowledge_v2_repository().ok_or_else(|| {
            "Knowledge requires the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let summary = repo
            .delete_chunk_global(id)
            .await
            .map_err(|e| e.to_string())?;
        self.notice = Some(format!(
            "Deleted chunk {id} (orphaned entities {}, relationships {})",
            summary.orphan_entity_count, summary.orphan_relationship_count
        ));
        self.reload().await
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KnowledgeAction {
        match &self.view {
            View::ConfirmDelete { chunk_id } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => KnowledgeAction::Delete(*chunk_id),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    KnowledgeAction::CancelDelete
                }
                _ => KnowledgeAction::None,
            },
            View::ChunkDetail { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view = View::List;
                    KnowledgeAction::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let View::ChunkDetail { scroll, .. } = &mut self.view {
                        *scroll = scroll.saturating_sub(1);
                    }
                    KnowledgeAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let View::ChunkDetail { scroll, .. } = &mut self.view {
                        *scroll = scroll.saturating_add(1);
                    }
                    KnowledgeAction::None
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let View::ChunkDetail { detail, .. } = &self.view {
                        KnowledgeAction::ConfirmDelete(detail.chunk.id)
                    } else {
                        KnowledgeAction::None
                    }
                }
                _ => KnowledgeAction::None,
            },
            View::EntityDetail { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view = View::List;
                    KnowledgeAction::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let View::EntityDetail { scroll, .. } = &mut self.view {
                        *scroll = scroll.saturating_sub(1);
                    }
                    KnowledgeAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let View::EntityDetail { scroll, .. } = &mut self.view {
                        *scroll = scroll.saturating_add(1);
                    }
                    KnowledgeAction::None
                }
                _ => KnowledgeAction::None,
            },
            View::List => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => KnowledgeAction::Close,
                KeyCode::Char('r') | KeyCode::Char('R') => KnowledgeAction::Reload,
                KeyCode::Char('m') | KeyCode::PageDown => KnowledgeAction::LoadMore,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected = self.selected.saturating_sub(1);
                    KnowledgeAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.items.is_empty() {
                        self.selected = (self.selected + 1).min(self.items.len() - 1);
                    }
                    KnowledgeAction::None
                }
                KeyCode::Enter => {
                    if let Some(row) = self.items.get(self.selected) {
                        KnowledgeAction::OpenChunk(row.id)
                    } else {
                        KnowledgeAction::None
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(row) = self.items.get(self.selected) {
                        KnowledgeAction::ConfirmDelete(row.id)
                    } else {
                        KnowledgeAction::None
                    }
                }
                _ => KnowledgeAction::None,
            },
        }
    }

    pub fn begin_confirm_delete(&mut self, chunk_id: i32) {
        self.view = View::ConfirmDelete { chunk_id };
    }

    pub fn cancel_confirm_delete(&mut self) {
        self.view = View::List;
        self.notice = Some("Delete cancelled".into());
    }
}

fn build_preview(content: &str) -> String {
    let mut preview = String::new();
    let mut used = 0usize;
    for token in content.split_whitespace() {
        if !preview.is_empty() {
            if used >= PREVIEW_LEN {
                preview.push_str("...");
                return preview;
            }
            preview.push(' ');
            used += 1;
        }
        for ch in token.chars() {
            if used >= PREVIEW_LEN {
                preview.push_str("...");
                return preview;
            }
            preview.push(ch);
            used += 1;
        }
    }
    preview
}

pub fn draw_knowledge(frame: &mut Frame<'_>, screen: &KnowledgeScreen) {
    match &screen.view {
        View::ConfirmDelete { chunk_id } => {
            draw_list(frame, screen);
            let area = centered_rect(60, 5, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(format!(
                    " Delete knowledge chunk {chunk_id} permanently?\n y confirm · n/Esc cancel "
                ))
                .block(Block::default().borders(Borders::ALL).title(" confirm ")),
                area,
            );
        }
        View::ChunkDetail { detail, scroll } => {
            let title = format!(
                " knowledge chunk #{} · {} ",
                detail.chunk.id, detail.chunk.assistant_id
            );
            let mut lines = vec![
                format!(
                    "source: {}",
                    detail.chunk.source.as_deref().unwrap_or("(none)")
                ),
                format!(
                    "tags: {}",
                    parse_db_tags(detail.chunk.tags.as_ref()).join(", ")
                ),
                String::new(),
                detail.chunk.content.clone(),
                String::new(),
                "entities:".into(),
            ];
            for e in &detail.entities {
                lines.push(format!(
                    "  #{} {} ({})",
                    e.id,
                    e.name,
                    e.entity_type.as_deref().unwrap_or("?")
                ));
            }
            lines.push(String::new());
            lines.push("relationships:".into());
            for r in &detail.relationships {
                lines.push(format!(
                    "  {} #{} → #{}",
                    r.relation_type, r.source_entity_id, r.target_entity_id
                ));
            }
            let body = lines.join("\n");
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(3),
                    Constraint::Length(1),
                ])
                .split(frame.area());
            frame.render_widget(Paragraph::new(title), chunks[0]);
            frame.render_widget(
                Paragraph::new(body)
                    .wrap(Wrap { trim: false })
                    .scroll((*scroll, 0))
                    .block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(" ↑/↓ scroll · d delete · Esc back "),
                chunks[2],
            );
        }
        View::EntityDetail {
            entity,
            related,
            scroll,
        } => {
            let title = format!(" knowledge entity #{} · {} ", entity.id, entity.name);
            let mut lines = vec![
                format!("assistant: {}", entity.assistant_id),
                format!(
                    "type: {}",
                    entity.entity_type.as_deref().unwrap_or("(none)")
                ),
                format!(
                    "description: {}",
                    entity.description.as_deref().unwrap_or("(none)")
                ),
                String::new(),
                "related:".into(),
            ];
            lines.extend(related.iter().cloned());
            let body = lines.join("\n");
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(3),
                    Constraint::Length(1),
                ])
                .split(frame.area());
            frame.render_widget(Paragraph::new(title), chunks[0]);
            frame.render_widget(
                Paragraph::new(body)
                    .wrap(Wrap { trim: false })
                    .scroll((*scroll, 0))
                    .block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
            frame.render_widget(Paragraph::new(" ↑/↓ scroll · Esc back to list "), chunks[2]);
        }
        View::List => draw_list(frame, screen),
    }
}

fn draw_list(frame: &mut Frame<'_>, screen: &KnowledgeScreen) {
    let assistant = screen.assistant_id.as_deref().unwrap_or("all");
    let q = screen.query.as_deref().unwrap_or("");
    let title = format!(
        " /knowledge · {} chunks · assistant: {assistant} · q:{q} ",
        screen.items.len()
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());
    frame.render_widget(Paragraph::new(title), chunks[0]);

    let items: Vec<ListItem> = screen
        .items
        .iter()
        .map(|row| {
            ListItem::new(format!(
                "#{} [{}] {} {}",
                row.id,
                row.assistant_id,
                row.preview,
                if row.tags.is_empty() {
                    String::new()
                } else {
                    format!("#{}", row.tags.join("#"))
                }
            ))
        })
        .collect();
    let mut state = ListState::default();
    if !screen.items.is_empty() {
        state.select(Some(screen.selected));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" chunks "))
            .highlight_symbol("> "),
        chunks[1],
        &mut state,
    );
    let more = if screen.next_cursor.is_some() {
        " · m/PgDn more"
    } else {
        ""
    };
    let notice = screen.notice.as_deref().unwrap_or("");
    frame.render_widget(
        Paragraph::new(format!(
            " ↑/↓ · Enter open · d delete · r reload{more} · Esc close  {notice}"
        )),
        chunks[2],
    );
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height.min(100)) / 2),
            Constraint::Length(height),
            Constraint::Percentage((100 - height.min(100)) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup[1])[1]
}

/// Run a knowledge slash command against the in-process repository.
pub async fn run_knowledge_command(command: KnowledgeCommand) -> Result<KnowledgeScreen, String> {
    match command {
        KnowledgeCommand::Open {
            query,
            assistant_id,
        } => KnowledgeScreen::open(query, assistant_id).await,
        KnowledgeCommand::Entity { id } => KnowledgeScreen::open_entity(id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_open_search_and_assistant() {
        assert_eq!(
            parse_knowledge_command("/knowledge"),
            Some(KnowledgeCommand::Open {
                query: None,
                assistant_id: None
            })
        );
        assert_eq!(
            parse_knowledge_command("/knowledge search foo bar"),
            Some(KnowledgeCommand::Open {
                query: Some("foo bar".into()),
                assistant_id: None
            })
        );
        assert_eq!(
            parse_knowledge_command("/knowledge --assistant asst_1"),
            Some(KnowledgeCommand::Open {
                query: None,
                assistant_id: Some("asst_1".into())
            })
        );
        assert_eq!(
            parse_knowledge_command("/knowledge search notes --assistant asst_1"),
            Some(KnowledgeCommand::Open {
                query: Some("notes".into()),
                assistant_id: Some("asst_1".into())
            })
        );
        assert_eq!(
            parse_knowledge_command("/knowledge entity 42"),
            Some(KnowledgeCommand::Entity { id: 42 })
        );
        assert_eq!(parse_knowledge_command("/settings"), None);
        assert_eq!(parse_knowledge_command("//knowledge"), None);
    }

    #[test]
    fn attached_mode_refuses_without_repo() {
        // Unit tests do not boot repositories.
        let err = tauri::async_runtime::block_on(KnowledgeScreen::open(None, None)).unwrap_err();
        assert!(err.contains("primary backend") || err.contains("attached"));
    }
}
