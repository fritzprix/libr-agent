#!/usr/bin/env node

/**
 * manage-logs.js
 * 
 * Utility to extract logs from the system-specific Tauri log folder to the project directory.
 * Supports extracting the last N lines or extracting errors with context.
 * 
 * Usage:
 *   node scripts/manage-logs.js [--error] [-n=N]
 */

import fs from 'fs';
import path from 'path';
import os from 'os';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Application configuration from tauri.conf.json
const APP_NAME = 'LibrAgent';
const APP_ID = 'com.fritzprix.libragent';
const LOG_FILE_NAME = 'libragent.log';

/**
 * Gets the platform-specific path where Tauri stores logs
 */
function getLogSourcePath() {
    const platform = os.platform();
    const home = os.homedir();

    if (platform === 'win32') {
        // Windows: %LOCALAPPDATA%\<app-id>\logs\<app-name>.log
        const localAppData = process.env.LOCALAPPDATA || path.join(home, 'AppData', 'Local');
        return path.join(localAppData, APP_ID, 'logs', LOG_FILE_NAME);
    } else if (platform === 'darwin') {
        // macOS: ~/Library/Logs/<app-id>/<app-name>.log
        return path.join(home, 'Library', 'Logs', APP_ID, LOG_FILE_NAME);
    } else {
        // Linux: ~/.local/share/<app-id>/logs/<app-name>.log
        return path.join(home, '.local', 'share', APP_ID, 'logs', LOG_FILE_NAME);
    }
}

/**
 * Extracts lines containing a specific pattern with surrounding context
 */
function extractByPattern(lines, pattern = '[ERROR]', contextCount = 5) {
    const matchRanges = [];

    // Find all matches for the pattern
    for (let i = 0; i < lines.length; i++) {
        if (lines[i].includes(pattern)) {
            const start = Math.max(0, i - contextCount);
            const end = Math.min(lines.length - 1, i + contextCount);
            matchRanges.push({ start, end, matchIdx: i });
        }
    }

    if (matchRanges.length === 0) return [`No matches found for pattern "${pattern}" in the selected range.`];

    // Merge overlapping ranges
    const mergedRanges = [];
    if (matchRanges.length > 0) {
        let current = { ...matchRanges[0] };
        for (let i = 1; i < matchRanges.length; i++) {
            if (matchRanges[i].start <= current.end + 1) {
                current.end = Math.max(current.end, matchRanges[i].end);
            } else {
                mergedRanges.push(current);
                current = { ...matchRanges[i] };
            }
        }
        mergedRanges.push(current);
    }

    // Format output
    const result = [];
    mergedRanges.forEach((range, idx) => {
        result.push(`=== Match ${idx + 1}: Lines ${range.start + 1}-${range.end + 1} ===`);
        for (let i = range.start; i <= range.end; i++) {
            const lineNum = (i + 1).toString().padStart(6, ' ');
            const marker = lines[i].includes(pattern) ? ' > ' : '   ';
            result.push(`${lineNum}${marker}${lines[i].trimEnd()}`);
        }
        result.push('');
    });

    return result;
}


function main() {
    const args = process.argv.slice(2);

    // Parse options
    const isErrorMode = args.includes('--error') || process.env.npm_lifecycle_event === 'error';
    const isHelpMode = args.includes('--help') || args.includes('-h');
    const isStdoutMode = args.includes('--stdout');

    // Help message
    if (isHelpMode) {
        console.log(`
\x1b[1m📜 LibrAgent Log Manager\x1b[0m

\x1b[36mUsage:\x1b[0m
  pnpm log [options]
  pnpm error [options]

\x1b[36mOptions:\x1b[0m
  \x1b[33m-n <number>\x1b[0m      Number of lines to extract (default: 100 for log, 5000 for error)
  \x1b[33m--pattern=<text>\x1b[0m  Search for lines containing <text> and show context
  \x1b[33m--error\x1b[0m           Shortcut for --pattern="[ERROR]"
  \x1b[33m--stdout\x1b[0m          Print output to console instead of writing to a file
  \x1b[33m--help, -h\x1b[0m        Show this help message

\x1b[36mExamples:\x1b[0m
  pnpm log -n 50              \x1b[90m# Get last 50 lines of log\x1b[0m
  pnpm error                  \x1b[90m# Get all errors with context\x1b[0m
  pnpm log --pattern="PLAN"   \x1b[90m# Search for "PLAN" with context\x1b[0m
  pnpm log -n 20 --stdout     \x1b[90m# Print last 20 lines to console\x1b[0m
        `);
        process.exit(0);
    }

    // Parse pattern argument (e.g., --pattern="PLANNING")
    const patternArg = args.find(a => a.startsWith('--pattern='));
    const pattern = patternArg ? patternArg.split('=')[1] : '[ERROR]';

    // Use pattern mode if --error or --pattern is provided
    const isPatternMode = isErrorMode || !!patternArg;

    // Parse lines argument (e.g., -n=50, -n 50, or --lines=50)
    let lineCount;
    const nIndex = args.findIndex(a => a === '-n');
    if (nIndex !== -1 && args[nIndex + 1]) {
        lineCount = parseInt(args[nIndex + 1], 10);
    } else {
        const linesArg = args.find(a => a.startsWith('-n=') || a.startsWith('--lines='));
        const defaultLines = isPatternMode ? 5000 : 100;
        lineCount = linesArg ? parseInt(linesArg.split('=')[1], 10) : defaultLines;
    }

    const src = getLogSourcePath();
    const projectRoot = path.resolve(__dirname, '..');
    const now = new Date();
    const dateSuffix = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
    const baseName = isPatternMode ? (pattern.includes('ERROR') ? 'error' : 'log') : 'log';
    const destFileName = `${baseName}_${dateSuffix}.txt`;
    const dest = path.join(projectRoot, destFileName);

    console.log(`\x1b[36m🔍 Searching for logs at: ${src}\x1b[0m`);

    if (!fs.existsSync(src)) {
        console.error(`\x1b[31m❌ Log file does not exist at ${src}\x1b[0m`);
        console.log(`\x1b[33m💡 Try running the Tauri app first to generate logs.\x1b[0m`);
        process.exit(1);
    }

    try {
        const content = fs.readFileSync(src, 'utf8');
        const lines = content.split(/\r?\n/);

        let outputText;
        if (isPatternMode) {
            console.log(`\x1b[36m📊 Extracting matches for "${pattern}" with context from last ${lineCount} lines...\x1b[0m`);
            const searchScope = lines.slice(-lineCount);
            outputText = extractByPattern(searchScope, pattern).join('\n');
        } else {
            console.log(`\x1b[36m📊 Extracting last ${lineCount} lines...\x1b[0m`);
            outputText = lines.slice(-lineCount).join('\n');
        }

        if (isStdoutMode) {
            console.log(`\x1b[32m📖 Log Content:\x1b[0m\n`);
            console.log(outputText);
        } else {
            fs.writeFileSync(dest, outputText, 'utf8');
            console.log(`\x1b[32m✅ Successfully extracted to ${dest}\x1b[0m`);

            const outputLineCount = outputText.split('\n').length;
            console.log(`\x1b[32m📈 Total lines in output: ${outputLineCount}\x1b[0m`);
        }
    } catch (err) {
        console.error(`\x1b[31m❌ Failed to process logs: ${err.message}\x1b[0m`);
        process.exit(1);
    }
}

main();
