function close(w) { if (w.pid === 68352) w.closeWindow(); } workspace.windowAdded.connect(close); for (const w of workspace.windowList()) close(w);
