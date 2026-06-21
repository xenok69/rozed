local debris = game:GetService("Debris")
local HttpService = game:GetService("HttpService")
local PORT = 5500

local toolbar = plugin:CreateToolbar("Rozed")
local toggleButton = toolbar:CreateButton("Rozed", "Toggle Rozed panel", "")
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

local mainFrame = script:FindFirstChild("Background")

local screen = mainFrame:Clone()
screen.Parent = widget
local logFrame = screen:FindFirstChild("Logs")
local logContainer = logFrame:FindFirstChild("Container")

local connectFrame = screen:FindFirstChild("Connect")
local connectBtns = connectFrame:FindFirstChild("Buttons")
local connectBtn = connectBtns:FindFirstChild("ConnectBtn")
local connectStatus = connectFrame:FindFirstChild("Status")

local function log(level, message)
	local label = script:FindFirstChild("Template"):Clone()
	label.AutomaticSize = Enum.AutomaticSize.Y
	label.Text = string.format("[%s] %s", level, message)
	label.TextColor3 = level == "ERROR" and Color3.fromRGB(232, 116, 138)
		or level == "SUCCESS" and Color3.fromRGB(100, 220, 130)
		or Color3.fromRGB(255, 255, 255)
	label.Parent = logContainer

	debris:AddItem(label, 600)
end

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

	log("PUSH", `{scriptName} ({className}) -> {roblox_path}`)
end

local pollingActive = false
local connected = false

local function processEvent(data)
	if data.type == "script-pushed" then
		applyScript(data.name, data.kind, data.roblox_path, data.source)
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

log("SUCCESS", "Plugin loaded. Click Connect to start.")
