use crate::error::Result;
use crate::types::{TaskInfo, TaskStatus, TenantId};
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use tokio::sync::mpsc;

pub struct TaskQueue {
    pub(crate) sender: mpsc::Sender<TaskCommand>,
}

pub enum TaskCommand {
    Export {
        task_id: String,
        tenant_id: TenantId,
        dest_path: PathBuf,
    },
}

impl TaskQueue {
    pub fn new(manager: Weak<crate::IndexManager>, tasks: Arc<DashMap<String, TaskInfo>>) -> Self {
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(process_tasks(rx, tasks.clone(), manager));

        TaskQueue { sender: tx }
    }

    pub async fn enqueue_export(
        &self,
        task_id: String,
        tenant_id: TenantId,
        dest_path: PathBuf,
    ) -> Result<()> {
        self.sender
            .send(TaskCommand::Export {
                task_id,
                tenant_id,
                dest_path,
            })
            .await
            .map_err(|_| crate::FlapjackError::QueueFull)
    }
}

async fn process_tasks(
    mut rx: mpsc::Receiver<TaskCommand>,
    tasks: Arc<DashMap<String, TaskInfo>>,
    manager_weak: Weak<crate::IndexManager>,
) {
    while let Some(cmd) = rx.recv().await {
        let manager = match manager_weak.upgrade() {
            Some(m) => m,
            None => {
                let TaskCommand::Export { task_id, .. } = cmd;
                tasks.alter(&task_id, |_, mut t| {
                    t.status = TaskStatus::Failed("Manager dropped".to_string());
                    t
                });
                break;
            }
        };

        let TaskCommand::Export {
            task_id,
            tenant_id,
            dest_path,
        } = cmd;
        process_export(task_id, tenant_id, dest_path, manager, tasks.clone()).await;
    }
}

async fn process_export(
    _task_id: String,
    _tenant_id: TenantId,
    _dest_path: PathBuf,
    _manager: Arc<crate::IndexManager>,
    _tasks: Arc<DashMap<String, TaskInfo>>,
) {
    _tasks.alter(&_task_id, |_, mut t| {
        t.status = TaskStatus::Processing;
        t
    });

    _manager.write_queues.remove(&_tenant_id);

    if let Some((_, handle)) = _manager.write_task_handles.remove(&_tenant_id) {
        match handle.await {
            Ok(Ok(())) => (),
            Ok(Err(e)) => {
                _tasks.alter(&_task_id, |_, mut t| {
                    t.status = TaskStatus::Failed(format!("Commit failed: {}", e));
                    t
                });
                return;
            }
            Err(e) => {
                _tasks.alter(&_task_id, |_, mut t| {
                    t.status = TaskStatus::Failed(format!("Write task panicked: {:?}", e));
                    t
                });
                return;
            }
        }
    }

    let src = _manager.base_path.join(&_tenant_id);
    let dest = _dest_path.clone();

    let copy_result = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dest)?;
        crate::index::utils::copy_dir_recursive(&src, &dest)
    })
    .await;

    _manager.writers.remove(&_tenant_id);
    _manager.loaded.remove(&_tenant_id);

    match copy_result {
        Ok(Ok(())) => {
            _tasks.alter(&_task_id, |_, mut t| {
                t.status = TaskStatus::Succeeded;
                t
            });
        }
        Ok(Err(e)) => {
            _manager.writers.remove(&_tenant_id);
            _manager.loaded.remove(&_tenant_id);
            _tasks.alter(&_task_id, |_, mut t| {
                t.status = TaskStatus::Failed(format!("Copy failed: {}", e));
                t
            });
        }
        Err(e) => {
            _manager.writers.remove(&_tenant_id);
            _manager.loaded.remove(&_tenant_id);
            _tasks.alter(&_task_id, |_, mut t| {
                t.status = TaskStatus::Failed(format!("Spawn blocking failed: {:?}", e));
                t
            });
        }
    }
}
