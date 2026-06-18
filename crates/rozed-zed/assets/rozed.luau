local HttpService = game:GetService("HttpService")
local PORT = 5500

local toolbar = plugin:CreateToolbar("Rozed")
local toggleButton = toolbar:CreateButton("RozedToggle", "Toggle Rozed panel", "")
toggleButton.ClickableWhenViewportHidden = true

local widgetInfo = DockWidgetPluginGuiInfo.new(
    Enum.InitialDockState.Right,
    false, false, 300, 500, 200, 300
)
local widget = plugin:CreateDockWidgetPluginGuiAsync("RozedPanel", widgetInfo)
widget.Title = "ROZED"
widget.Name = "RozedPanel"
widget.ZIndexBehavior = Enum.ZIndexBehavior.Sibling

toggleButton.Click:Connect(function()
    widget.Enabled = not widget.Enabled
end)

widget:GetPropertyChangedSignal("Enabled"):Connect(function()
    toggleButton:SetActive(widget.Enabled)
end)

local screen = Instance.new("Frame")
screen.Size = UDim2.fromScale(1, 1)
screen.BackgroundColor3 = Color3.fromRGB(25, 25, 30)
screen.BorderSizePixel = 0
screen.Parent = widget

local header = Instance.new("Frame")
header.Size = UDim2.new(1, 0, 0, 40)
header.BackgroundColor3 = Color3.fromRGB(232, 116, 138)
header.BorderSizePixel = 0
header.Parent = screen

local titleLabel = Instance.new("TextLabel")
titleLabel.Size = UDim2.fromScale(1, 1)
titleLabel.BackgroundTransparency = 1
titleLabel.Text = "ROZED"
titleLabel.TextColor3 = Color3.fromRGB(255, 255, 255)
titleLabel.Font = Enum.Font.Code
titleLabel.TextSize = 16
titleLabel.Parent = header

local statusLabel = Instance.new("TextLabel")
statusLabel.Size = UDim2.new(1, -16, 0, 24)
statusLabel.Position = UDim2.new(0, 8, 0, 48)
statusLabel.BackgroundTransparency = 1
statusLabel.Text = "[DISCONNECTED]"
statusLabel.TextColor3 = Color3.fromRGB(180, 180, 180)
statusLabel.Font = Enum.Font.Code
statusLabel.TextSize = 12
statusLabel.TextXAlignment = Enum.TextXAlignment.Left
statusLabel.Parent = screen

local connectBtn = Instance.new("TextButton")
connectBtn.Size = UDim2.new(1, -16, 0, 32)
connectBtn.Position = UDim2.new(0, 8, 0, 80)
connectBtn.BackgroundColor3 = Color3.fromRGB(232, 116, 138)
connectBtn.BorderSizePixel = 0
connectBtn.Text = "Connect"
connectBtn.TextColor3 = Color3.fromRGB(255, 255, 255)
connectBtn.Font = Enum.Font.Code
connectBtn.TextSize = 13
connectBtn.Parent = screen

local pullBtn = Instance.new("TextButton")
pullBtn.Size = UDim2.new(1, -16, 0, 32)
pullBtn.Position = UDim2.new(0, 8, 0, 120)
pullBtn.BackgroundColor3 = Color3.fromRGB(50, 50, 60)
pullBtn.BorderSizePixel = 0
pullBtn.Text = "Pull from Zed"
pullBtn.TextColor3 = Color3.fromRGB(200, 200, 200)
pullBtn.Font = Enum.Font.Code
pullBtn.TextSize = 13
pullBtn.Parent = screen

local logFrame = Instance.new("ScrollingFrame")
logFrame.Size = UDim2.new(1, -16, 1, -168)
logFrame.Position = UDim2.new(0, 8, 0, 160)
logFrame.BackgroundColor3 = Color3.fromRGB(15, 15, 18)
logFrame.BorderSizePixel = 0
logFrame.ScrollBarThickness = 4
logFrame.CanvasSize = UDim2.new(1, 0, 0, 0)
logFrame.AutomaticCanvasSize = Enum.AutomaticSize.Y
logFrame.Parent = screen

local logLayout = Instance.new("UIListLayout")
logLayout.Padding = UDim.new(0, 2)
logLayout.Parent = logFrame

local function log(level, message)
    local label = Instance.new("TextLabel")
    label.Size = UDim2.new(1, -8, 0, 0)
    label.AutomaticSize = Enum.AutomaticSize.Y
    label.BackgroundTransparency = 1
    label.Text = string.format("[%s] %s", level, message)
    label.TextColor3 = level == "ERROR" and Color3.fromRGB(232, 116, 138)
        or level == "SUCCESS" and Color3.fromRGB(100, 220, 130)
        or Color3.fromRGB(180, 180, 180)
    label.Font = Enum.Font.Code
    label.TextSize = 11
    label.TextXAlignment = Enum.TextXAlignment.Left
    label.TextWrapped = true
    label.Parent = logFrame
end

-- Script kind -> Roblox class name
local TYPE_MAP = {
    ModuleScript = "ModuleScript",
    Script = "Script",
    LocalScript = "LocalScript",
}

local function applyScript(name, kind, roblox_path, source)
    local parts = string.split(roblox_path, "/")
    local parent = game
    for i = 1, #parts - 1 do
        local part = parts[i]
        local child = parent:FindFirstChild(part)
        if not child then
            child = Instance.new("Folder")
            child.Name = part
            child.Parent = parent
        end
        parent = child
    end

    local scriptName = parts[#parts]
    local className = TYPE_MAP[kind] or "ModuleScript"
    local existing = parent:FindFirstChild(scriptName)

    if existing and existing:IsA(className) then
        existing.Source = source
    else
        if existing then existing:Destroy() end
        local inst = Instance.new(className)
        inst.Name = scriptName
        inst.Source = source
        inst.Parent = parent
    end
    log("PUSH", scriptName .. " -> " .. roblox_path)
end

local handlePullReady
local handleStructureMissing

local pollingActive = false
local connected = false

local function processEvent(data)
    if data.type == "script-pushed" then
        applyScript(data.name, data.kind, data.roblox_path, data.source)
    elseif data.type == "pull-ready" then
        handlePullReady(data.files)
    elseif data.type == "structure-missing" then
        handleStructureMissing(data.paths)
    elseif data.type == "error" then
        log("ERROR", data.message)
    end
end

local function stopPolling()
    pollingActive = false
    connected = false
    statusLabel.Text = "[DISCONNECTED]"
    connectBtn.Text = "Connect"
    log("INFO", "Disconnected")
end

local function startPolling()
    local ok, _ = pcall(function()
        HttpService:GetAsync(string.format("http://localhost:%d/status", PORT), true)
    end)
    if not ok then
        log("ERROR", "Could not reach rozed on port " .. PORT .. ". Is it running?")
        return
    end

    pollingActive = true
    connected = true
    statusLabel.Text = string.format("[CONNECTED] localhost:%d", PORT)
    connectBtn.Text = "Disconnect"
    log("CONNECTED", string.format("localhost:%d", PORT))

    task.spawn(function()
        while pollingActive do
            local pok, result = pcall(function()
                return HttpService:GetAsync(
                    string.format("http://localhost:%d/events/poll", PORT),
                    true
                )
            end)
            if pok then
                local dok, events = pcall(function()
                    return HttpService:JSONDecode(result)
                end)
                if dok and type(events) == "table" then
                    for _, event in ipairs(events) do
                        processEvent(event)
                    end
                end
            else
                log("ERROR", "Lost connection: " .. tostring(result))
                stopPolling()
                break
            end
            task.wait(0.5)
        end
    end)
end

connectBtn.MouseButton1Click:Connect(function()
    if connected then
        stopPolling()
    else
        startPolling()
    end
end)

local function confirmOverwrite(name)
    local result = false
    local done = false

    local gui = Instance.new("ScreenGui")
    gui.Name = "RozedConfirm"
    gui.ResetOnSpawn = false
    gui.Parent = game:GetService("CoreGui")

    local frame = Instance.new("Frame")
    frame.Size = UDim2.new(0, 420, 0, 180)
    frame.Position = UDim2.fromScale(0.5, 0.5)
    frame.AnchorPoint = Vector2.new(0.5, 0.5)
    frame.BackgroundColor3 = Color3.fromRGB(25, 25, 30)
    frame.BorderSizePixel = 0
    frame.Parent = gui

    local msg = Instance.new("TextLabel")
    msg.Size = UDim2.new(1, -16, 0, 90)
    msg.Position = UDim2.new(0, 8, 0, 12)
    msg.BackgroundTransparency = 1
    msg.Text = string.format(
        "[CONFLICT] %s differs from Roblox version.\nOverwrite local file with Roblox version?",
        name
    )
    msg.TextColor3 = Color3.fromRGB(220, 220, 220)
    msg.Font = Enum.Font.Code
    msg.TextSize = 13
    msg.TextWrapped = true
    msg.TextXAlignment = Enum.TextXAlignment.Left
    msg.Parent = frame

    local yesBtn = Instance.new("TextButton")
    yesBtn.Size = UDim2.new(0, 180, 0, 36)
    yesBtn.Position = UDim2.new(0, 8, 1, -48)
    yesBtn.BackgroundColor3 = Color3.fromRGB(232, 116, 138)
    yesBtn.BorderSizePixel = 0
    yesBtn.Text = "Overwrite"
    yesBtn.TextColor3 = Color3.fromRGB(255, 255, 255)
    yesBtn.Font = Enum.Font.Code
    yesBtn.TextSize = 13
    yesBtn.Parent = frame

    local noBtn = Instance.new("TextButton")
    noBtn.Size = UDim2.new(0, 180, 0, 36)
    noBtn.Position = UDim2.new(1, -188, 1, -48)
    noBtn.BackgroundColor3 = Color3.fromRGB(50, 50, 60)
    noBtn.BorderSizePixel = 0
    noBtn.Text = "Skip"
    noBtn.TextColor3 = Color3.fromRGB(200, 200, 200)
    noBtn.Font = Enum.Font.Code
    noBtn.TextSize = 13
    noBtn.Parent = frame

    yesBtn.MouseButton1Click:Connect(function() result = true; done = true; gui:Destroy() end)
    noBtn.MouseButton1Click:Connect(function() done = true; gui:Destroy() end)

    while not done do task.wait(0.05) end
    return result
end

handlePullReady = function(files)
    local toWrite = {}
    for _, file in ipairs(files) do
        if file.conflict then
            log("CONFLICT", file.name .. " differs -- awaiting confirmation")
            if confirmOverwrite(file.name) then
                table.insert(toWrite, file)
            else
                log("INFO", "Skipped " .. file.name)
            end
        else
            table.insert(toWrite, file)
        end
    end

    if #toWrite == 0 then return end

    local ok, err = pcall(function()
        HttpService:PostAsync(
            string.format("http://localhost:%d/pull/confirm", PORT),
            HttpService:JSONEncode({ files = toWrite }),
            Enum.HttpContentType.ApplicationJson
        )
    end)
    if ok then
        log("SUCCESS", string.format("%d file(s) written", #toWrite))
    else
        log("ERROR", tostring(err))
    end
end

handleStructureMissing = function(paths)
    for _, path in ipairs(paths) do
        log("INIT", "Missing: " .. path)
        local parts = string.split(path, "/")
        local parent = game
        for _, part in ipairs(parts) do
            local child = parent:FindFirstChild(part)
            if not child then
                child = Instance.new("Folder")
                child.Name = part
                child.Parent = parent
                log("INIT", "Created " .. part)
            end
            parent = child
        end
    end
    log("SUCCESS", "Structure ready")
end

pullBtn.MouseButton1Click:Connect(function()
    if not connected then
        log("ERROR", "Not connected. Click Connect first.")
        return
    end

    log("PULL", "Fetching scripts from Roblox...")

    local statusOk, statusRaw = pcall(function()
        return HttpService:GetAsync(string.format("http://localhost:%d/status", PORT))
    end)
    if not statusOk then
        log("ERROR", "Could not reach backend: " .. tostring(statusRaw))
        return
    end

    local statusData = HttpService:JSONDecode(statusRaw)
    local allScripts = {}

    local function collectScripts(roblox_root)
        local parts = string.split(roblox_root, "/")
        local node = game
        for _, part in ipairs(parts) do
            node = node:FindFirstChild(part)
            if not node then return end
        end
        local function walk(inst, path)
            for _, child in ipairs(inst:GetChildren()) do
                local childPath = path .. "/" .. child.Name
                if child:IsA("ModuleScript") then
                    table.insert(allScripts, { roblox_path = childPath, name = child.Name, kind = "ModuleScript", source = child.Source })
                elseif child:IsA("LocalScript") then
                    table.insert(allScripts, { roblox_path = childPath, name = child.Name, kind = "LocalScript", source = child.Source })
                elseif child:IsA("Script") then
                    table.insert(allScripts, { roblox_path = childPath, name = child.Name, kind = "Script", source = child.Source })
                end
                walk(child, childPath)
            end
        end
        walk(node, roblox_root)
    end

    for _, roblox_path in pairs(statusData.mappings or {}) do
        collectScripts(roblox_path)
    end

    if #allScripts == 0 then
        log("INFO", "No scripts found in mapped Roblox paths")
        return
    end

    local ok, err = pcall(function()
        HttpService:PostAsync(
            string.format("http://localhost:%d/pull", PORT),
            HttpService:JSONEncode({ files = allScripts }),
            Enum.HttpContentType.ApplicationJson
        )
    end)
    if not ok then
        log("ERROR", tostring(err))
    end
end)

log("INFO", "Plugin loaded. Click Connect to start.")
