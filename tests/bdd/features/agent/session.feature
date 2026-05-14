Feature: Agent session lifecycle

  Scenario: Create a new agent session
    Given the SmartAssist gateway is running
    When I create a session named 'test-session' with model 'claude-sonnet-4-6'
    Then the response status should be 200
    And the response should contain 'test-session'

  Scenario: Run a task in an existing session
    Given the SmartAssist gateway is running
    And an agent session named 'test-session' exists
    When I run the task 'list files' in the session
    Then the response status should be 200
    And the session should receive a response
