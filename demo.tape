# tcc-rs TUI demo (~30 seconds)
Output demo.gif
Set Width 1200
Set Height 700
Set FontSize 15
Set Theme "Catppuccin Mocha"
Set Padding 10

# Start mock daemon in background, then launch TUI
Type "just run-mock-daemon &"
Enter
Sleep 3s
Type "just run-tui"
Enter
Sleep 2s

# Dashboard tab (default)
Sleep 1.5s

# Profiles tab
Type "2"
Sleep 2s

# Navigate profiles list
Down
Sleep 0.5s
Down
Sleep 0.5s
Down
Sleep 1s

# Fan Curve tab
Type "3"
Sleep 2s

# Navigate fan curve points
Right
Sleep 0.4s
Right
Sleep 0.4s
Right
Sleep 0.4s

# Adjust speed up on a point
Up
Sleep 0.3s
Up
Sleep 0.3s
Up
Sleep 0.3s
Up
Sleep 0.3s
Up
Sleep 0.5s

# Move to next point and adjust down
Right
Sleep 0.4s
Down
Sleep 0.3s
Down
Sleep 0.3s
Down
Sleep 0.5s

# Insert a new point
Type "i"
Sleep 1s

# Adjust the inserted point
Up
Sleep 0.3s
Up
Sleep 0.3s
Up
Sleep 0.5s

# Save the curve
Type "s"
Sleep 1.5s

# Settings tab
Type "4"
Sleep 2s

# Navigate settings fields
Down
Sleep 0.4s
Down
Sleep 0.4s
Right
Sleep 0.5s
Down
Sleep 0.4s
Right
Sleep 0.5s

# Keyboard tab
Type "5"
Sleep 1.5s

# Charging tab
Type "6"
Sleep 1.5s

# Power tab
Type "7"
Sleep 1.5s

# Display tab
Type "8"
Sleep 1.5s

# Webcam tab
Type "9"
Sleep 1.5s

# Info tab
Type "0"
Sleep 1.5s

# Show help overlay
Type "?"
Sleep 2s

# Close help
Escape
Sleep 1s

# Quit
Type "q"
Sleep 0.5s
