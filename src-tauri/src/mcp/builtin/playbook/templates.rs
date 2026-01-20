/// HTML template for playbook list UI with pagination
pub const PLAYBOOK_LIST_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset='utf-8'>
    <meta name='viewport' content='width=device-width,initial-scale=1'>
    <style>
        body {
            font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 16px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 15px;
            padding: 24px;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
        }
        h2 { margin-top: 0; text-align: center; }
        .playbook-item {
            background: rgba(255, 255, 255, 0.15);
            padding: 16px;
            margin: 12px 0;
            border-radius: 8px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .playbook-info { flex: 1; }
        .playbook-goal { font-weight: bold; font-size: 16px; }
        .playbook-meta { font-size: 12px; opacity: 0.7; margin-top: 4px; }
        .btn-group { display: flex; gap: 8px; }
        button {
            padding: 8px 16px;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-size: 14px;
            transition: all 0.2s;
        }
        button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .btn-select {
            background: linear-gradient(45deg, #2196F3, #21CBF3);
            color: white;
        }
        .btn-select:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(33, 150, 243, 0.4); }
        .btn-delete {
            background: linear-gradient(45deg, #f44336, #e91e63);
            color: white;
        }
        .btn-delete:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 4px 8px rgba(244, 67, 54, 0.4); }
        .pagination {
            display: flex;
            justify-content: center;
            align-items: center;
            gap: 16px;
            margin-top: 24px;
        }
        .page-btn {
            background: rgba(255, 255, 255, 0.2);
            color: white;
        }
        .page-btn:hover:not(:disabled) {
            background: rgba(255, 255, 255, 0.3);
        }
        .empty-state {
            text-align: center;
            padding: 40px;
            opacity: 0.8;
        }
    </style>
</head>
<body>
    <div class='container'>
        <h2>📚 Playbooks ({{totalItems}})</h2>
        {{#if hasPlaybooks}}
        <div id='playbook-list'>
            {{#each playbooks}}
            <div class='playbook-item'>
                <div class='playbook-info'>
                    <div class='playbook-goal'>{{this.goal}}</div>
                    <div class='playbook-meta'>
                        ID: {{this.id}} | Steps: {{this.step_count}} | Created: {{this.created_at_fmt}}
                    </div>
                </div>
                <div class='btn-group'>
                    <button class='btn-select' data-id='{{this.id}}'>Select</button>
                    <button class='btn-delete' data-id='{{this.id}}'>Delete</button>
                </div>
            </div>
            {{/each}}
        </div>
        <div class='pagination'>
            <button class='page-btn' id='prev-btn' {{prevDisabled}}>Previous</button>
            <span>Page {{page}} of {{totalPages}}</span>
            <button class='page-btn' id='next-btn' {{nextDisabled}}>Next</button>
        </div>
        {{else}}
        <div class='empty-state'>
            <p>No playbooks found</p>
            <p>Create your first playbook to get started!</p>
        </div>
        {{/if}}
    </div>
    <script>
        document.addEventListener('DOMContentLoaded', function() {
            // Select buttons
            document.querySelectorAll('.btn-select').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    const id = this.getAttribute('data-id');
                    window.parent.postMessage({
                        type: 'tool',
                        payload: {
                            toolName: 'selectPlaybook',
                            params: { id: id }
                        }
                    }, '*');
                });
            });
            // Delete buttons
            document.querySelectorAll('.btn-delete').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    const id = this.getAttribute('data-id');
                    if (confirm('Delete playbook "' + id + '"?')) {
                        window.parent.postMessage({
                            type: 'tool',
                            payload: {
                                toolName: 'deletePlaybook',
                                params: { id: id }
                            }
                        }, '*');
                    }
                });
            });
            // Pagination
            const page = {{page}};
            document.getElementById('prev-btn')?.addEventListener('click', function() {
                window.parent.postMessage({
                    type: 'tool',
                    payload: {
                        toolName: 'getPlaybookPage',
                        params: { page: page - 1, pageSize: {{pageSize}} }
                    }
                }, '*');
            });
            document.getElementById('next-btn')?.addEventListener('click', function() {
                window.parent.postMessage({
                    type: 'tool',
                    payload: {
                        toolName: 'getPlaybookPage',
                        params: { page: page + 1, pageSize: {{pageSize}} }
                    }
                }, '*');
            });
        });
    </script>
</body>
</html>"#;
