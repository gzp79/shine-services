- on portal create an `App registration`
  - It's enough to set it for `Accounts in this organizational directory only (Gzp only - Single tenan` as we are not planning to expose the app to the world.
  - take notes from the oOverview page:
	    application id will be the `AZURE_CLIENT_ID`
		tenant id will be the `AZURE_TENANT_ID`
  - Add a secret under `Certificates & secrets`, description could be "service" as it is used by the service
	- take a note about the generated secret, it will be the `AZURE_CLIENT_SECRET`. It can be accessed only at this time !
  - add some rule to it, otherwise it won't be assigned to the subscription (`No subscription found for`)
    - in the keyvault IAM (not Access policy!): Add the new service principal (registered app) with a `Key Vault Reader` role  
	
- to test from cli: `az login --service-principal -u $env:AZURE_CLIENT_ID -p $env:AZURE_CLIENT_SECRET --tenant $env:AZURE_TENANT_ID`
