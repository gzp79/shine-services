Feature: fetching User Details

  Background:
    * url 'http://test.scytta.com'
 
  Scenario: testing the get call for User Details
    * path '/info/ready'
    When method GET
    Then status 200