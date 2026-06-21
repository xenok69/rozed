local test = {}

function test.hello()
	print("Hello from test module.")
end

function test.update()
    print(`[{script.Name}] This is a test > should work hopefully`)
end

return test
