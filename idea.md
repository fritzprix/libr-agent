# Sprint W3-Jan-26

## file operations enhancement

- editFile
  - if the oldString and newString are identical, then return error clearly state that there is nothing to update
- new operations
  - searchLineInFile
    - search pattern in file, regex can be used, and exact match as well
    - return line number of match and the string the matched line
  - editLineInFile
    - replace the multiple lines with new string
    - [{line:number, value: string},]
    - return the diff state of updated block (+- 2 ~ 3 lines around the change, so that the agent understand the update is correct or not)

## workspace mounting and opening

- workspace is created when the session initiated with local app cache dir
- user can configure certain directory as workspace so that the workspace directory setting can be overridden
- new button to open the workspace in file explore and terminal, and override workspace directory in Workspace Panel
- remove buggy go to home button in Workspace Panel (it is not functional at this moment)
