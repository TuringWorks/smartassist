Feature: Browser automation

  Scenario: Navigate to a URL
    Given the SmartAssist gateway is running
    And a browser session named 'browser-1' exists
    When I navigate to 'https://example.com' in session 'browser-1'
    Then the response should contain 'success'
    And the current URL should be 'https://example.com'

  Scenario: Take a screenshot
    Given the SmartAssist gateway is running
    And a browser session named 'browser-1' exists
    When I take a screenshot in session 'browser-1'
    Then the response should contain 'screenshot'
    And the screenshot data should not be empty

  Scenario: Click an element
    Given the SmartAssist gateway is running
    And a browser session named 'browser-1' exists
    And the page contains a button with id 'submit'
    When I click the element with id 'submit' in session 'browser-1'
    Then the response should contain 'success'

  Scenario: Evaluate JavaScript
    Given the SmartAssist gateway is running
    And a browser session named 'browser-1' exists
    When I evaluate the script "document.title" in session 'browser-1'
    Then the response should contain 'result'
