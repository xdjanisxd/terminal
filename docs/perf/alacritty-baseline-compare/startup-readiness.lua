local recorded = false
clink.onbeginedit(function()
    if recorded then return end
    local path = os.getenv("STARTUP_COMPARISON_PROMPT")
    if not path then return end
    local file = io.open(path, "w")
    if file then file:write("clink-onbeginedit"); file:close(); recorded = true end
end)