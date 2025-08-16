onDragDropEvent()
onDragDropEvent(handler): Promise<UnlistenFn>

Listen to a file drop event. The listener is triggered when the user hovers the selected files on the webview, drops the files or cancels the operation.

Parameters
Parameter	Type
handler	EventCallback<DragDropEvent>
Returns
Promise<UnlistenFn>

A promise resolving to a function to unlisten to the event. Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.

Example
import { getCurrentWebview } from "@tauri-apps/api/webview";
const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
 if (event.payload.type === 'over') {
   console.log('User hovering', event.payload.position);
 } else if (event.payload.type === 'drop') {
   console.log('User dropped', event.payload.paths);
 } else {
   console.log('File drop cancelled');
 }
});

// you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
unlisten();

When the debugger panel is open, the drop position of this event may be inaccurate due to a known limitation. To retrieve the correct drop position, please detach the debugger.

Inherited from
Window.onDragDropEvent

Source: https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/webview.ts#L640


