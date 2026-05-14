Feature: Canvas workspace

  Scenario: Create a canvas surface
    Given the SmartAssist gateway is running
    When I create a canvas surface named 'canvas-1'
    Then the response should contain 'canvas-1'
    And the response status should be 200

  Scenario: Add an element to the canvas
    Given the SmartAssist gateway is running
    And a canvas surface named 'canvas-1' exists
    When I add a text element with id 'el-1' to surface 'canvas-1'
    Then the response should contain 'el-1'
    And the element count should be 1

  Scenario: Subscribe a client to a surface
    Given the SmartAssist gateway is running
    And a canvas surface named 'canvas-1' exists
    When I subscribe client 'client-1' to surface 'canvas-1'
    Then the response should contain 'success'
    And the subscriber count should be 1

  Scenario: Reset canvas surface
    Given the SmartAssist gateway is running
    And a canvas surface named 'canvas-1' exists
    And the surface has elements
    When I reset the surface 'canvas-1'
    Then the element count should be 0
