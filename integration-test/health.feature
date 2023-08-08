Feature: fetching User Details

  Background:
    * url karate.properties['serviceUrl']
 
  Scenario: testing the get call for User Details
    * path '/info/ready'
    When method GET
    Then status 200