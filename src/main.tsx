import { attachConsole } from "@tauri-apps/plugin-log";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { BrowserRouter } from "react-router";
import { ThemeProvider } from "next-themes";

// Initialize Tauri logger
attachConsole().catch(console.error);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <ThemeProvider attribute="class" defaultTheme="dark">
        <App />
      </ThemeProvider>
    </BrowserRouter>
  </React.StrictMode>,
);
