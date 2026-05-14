Feature: Tool execution

  Scenario: Read a file using the read tool
    Given the SmartAssist gateway is running
    And an agent session named 'test-session' exists
    When I invoke the 'read' tool with args:
      """
      {"path": "/tmp/test.txt"}
      """
    Then the tool result should contain 'content'

  Scenario: Execute a bash command using the bash tool
    Given the SmartAssist gateway is running
    And an agent session named 'test-session' exists
    When I invoke the 'bash' tool with args:
      """
      {"command": "echo hello"}
      """
    Then the tool result should contain 'hello'
