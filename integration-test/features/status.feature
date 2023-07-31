Feature: Service health

  Scenario: Checkking health
    Given a GET request to the base at "info/ready"
    When the response is received
    Then the response should have a status 200
    And the response should have a body
      """
      Ok
      """

  Scenario: Checkking status
    Given a GET request to the API at "health"
    When the response is received
    Then the response should have a status 200
    And the response should have a json schema
      """
      {
        "type": "object",
        "properties": {
          "postgres": {
            "type": "object",
            "properties": {
              "connections": {
                "type": "integer"
              },
              "idleConnections": {
                "type": "integer"
              }
            },
            "required": [
              "connections",
              "idleConnections"
            ]
          },
          "redis": {
            "type": "object",
            "properties": {
              "connections": {
                "type": "integer"
              },
              "idleConnections": {
                "type": "integer"
              }
            },
            "required": [
              "connections",
              "idleConnections"
            ]
          }
        },
        "required": [
          "postgres",
          "redis"
        ]
      }
      """
