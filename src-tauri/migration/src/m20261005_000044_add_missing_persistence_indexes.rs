use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // list_todos filters by session and orders by (created_at, id).
        manager
            .create_index(
                Index::create()
                    .name("idx-planning_todos-session_created_id")
                    .table(PlanningTodos::Table)
                    .col(PlanningTodos::SessionId)
                    .col(PlanningTodos::CreatedAt)
                    .col(PlanningTodos::Id)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // get_active_goal / archive filters on (session_id, status).
        manager
            .create_index(
                Index::create()
                    .name("idx-planning_goals-session_status")
                    .table(PlanningGoals::Table)
                    .col(PlanningGoals::SessionId)
                    .col(PlanningGoals::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Recursive graph walks join relationships on assistant_id plus either endpoint.
        manager
            .create_index(
                Index::create()
                    .name("idx-knowledge_relationships-assistant_source")
                    .table(KnowledgeRelationships::Table)
                    .col(KnowledgeRelationships::AssistantId)
                    .col(KnowledgeRelationships::SourceEntityId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-knowledge_relationships-assistant_target")
                    .table(KnowledgeRelationships::Table)
                    .col(KnowledgeRelationships::AssistantId)
                    .col(KnowledgeRelationships::TargetEntityId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // list_due_tasks: enabled = true AND next_run_at <= now.
        manager
            .create_index(
                Index::create()
                    .name("idx-scheduled_tasks-enabled_next_run")
                    .table(ScheduledTasks::Table)
                    .col(ScheduledTasks::Enabled)
                    .col(ScheduledTasks::NextRunAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-scheduled_tasks-enabled_next_run")
                    .table(ScheduledTasks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx-knowledge_relationships-assistant_target")
                    .table(KnowledgeRelationships::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx-knowledge_relationships-assistant_source")
                    .table(KnowledgeRelationships::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx-planning_goals-session_status")
                    .table(PlanningGoals::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx-planning_todos-session_created_id")
                    .table(PlanningTodos::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum PlanningTodos {
    #[sea_orm(iden = "planning_todos")]
    Table,
    SessionId,
    CreatedAt,
    Id,
}

#[derive(DeriveIden)]
enum PlanningGoals {
    #[sea_orm(iden = "planning_goals")]
    Table,
    SessionId,
    Status,
}

#[derive(DeriveIden)]
enum KnowledgeRelationships {
    #[sea_orm(iden = "knowledge_relationships")]
    Table,
    AssistantId,
    SourceEntityId,
    TargetEntityId,
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    Enabled,
    NextRunAt,
}
