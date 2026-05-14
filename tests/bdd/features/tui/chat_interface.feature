Feature: TUI Chat Interface
  The terminal UI provides a rich chat experience with message history,
  streaming responses, and slash commands.

  Scenario: Launch TUI command parses correctly
    When I run "smartassist tui --agent mybot"
    Then the CLI should parse the Tui command with agent "mybot"

  Scenario: Launch TUI with default agent
    When I run "smartassist tui"
    Then the CLI should parse the Tui command with no agent specified

  Scenario: TUI app initializes with empty chat
    Given a TUI app with agent "test-agent"
    Then the messages list should be empty
    And the input field should be empty
    And the status should show "Press Enter to send, Esc to quit"

  Scenario: User types a message in the input field
    Given a TUI app with agent "test-agent"
    When the user types "Hello world"
    Then the input field should contain "Hello world"

  Scenario: User sends a message
    Given a TUI app with agent "test-agent"
    When the user types "Hello"
    And the user presses Enter
    Then a User message "Hello" should appear in chat
    And the input field should be empty

  Scenario: User clears chat with slash command
    Given a TUI app with agent "test-agent"
    When the user sends "/clear"
    Then the messages list should be empty
