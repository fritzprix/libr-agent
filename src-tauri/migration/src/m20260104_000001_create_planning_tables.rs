use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create planning_goals table
        manager
            .create_table(
                Table::create()
                    .table(PlanningGoals::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlanningGoals::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlanningGoals::SessionId).string().not_null())
                    .col(ColumnDef::new(PlanningGoals::GoalText).string().not_null())
                    .col(
                        ColumnDef::new(PlanningGoals::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(PlanningGoals::CreatedAt)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_planning_goals_session")
                            .from(PlanningGoals::Table, PlanningGoals::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on session_id for planning_goals
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_planning_goals_session_id")
                    .table(PlanningGoals::Table)
                    .col(PlanningGoals::SessionId)
                    .to_owned(),
            )
            .await?;

        // Create planning_todos table
        manager
            .create_table(
                Table::create()
                    .table(PlanningTodos::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlanningTodos::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlanningTodos::SessionId).string().not_null())
                    .col(ColumnDef::new(PlanningTodos::Content).string().not_null())
                    .col(ColumnDef::new(PlanningTodos::Description).string())
                    .col(
                        ColumnDef::new(PlanningTodos::Priority)
                            .string()
                            .not_null()
                            .default("medium"),
                    )
                    .col(ColumnDef::new(PlanningTodos::ParentId).integer())
                    .col(
                        ColumnDef::new(PlanningTodos::IsChecked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(PlanningTodos::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(PlanningTodos::CreatedAt)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PlanningTodos::UpdatedAt)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_planning_todos_session")
                            .from(PlanningTodos::Table, PlanningTodos::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on session_id for planning_todos
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_planning_todos_session_id")
                    .table(PlanningTodos::Table)
                    .col(PlanningTodos::SessionId)
                    .to_owned(),
            )
            .await?;

        // Create planning_scratchpad table
        manager
            .create_table(
                Table::create()
                    .table(PlanningScratchpad::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlanningScratchpad::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlanningScratchpad::SessionId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlanningScratchpad::Content)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlanningScratchpad::Title).string())
                    .col(ColumnDef::new(PlanningScratchpad::Source).string())
                    .col(ColumnDef::new(PlanningScratchpad::Tags).string())
                    .col(
                        ColumnDef::new(PlanningScratchpad::CreatedAt)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PlanningScratchpad::UpdatedAt)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_planning_scratchpad_session")
                            .from(PlanningScratchpad::Table, PlanningScratchpad::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on session_id for planning_scratchpad
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_planning_scratchpad_session_id")
                    .table(PlanningScratchpad::Table)
                    .col(PlanningScratchpad::SessionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order (respecting foreign keys)
        manager
            .drop_table(Table::drop().table(PlanningScratchpad::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(PlanningTodos::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(PlanningGoals::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum PlanningGoals {
    Table,
    Id,
    SessionId,
    GoalText,
    Status,
    CreatedAt,
}

#[derive(Iden)]
enum PlanningTodos {
    Table,
    Id,
    SessionId,
    Content,
    Description,
    Priority,
    ParentId,
    IsChecked,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum PlanningScratchpad {
    Table,
    Id,
    SessionId,
    Content,
    Title,
    Source,
    Tags,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Sessions {
    Table,
    Id,
}
