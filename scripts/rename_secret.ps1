
# Check if the number of arguments is correct
if ($args.Length -ne 2) {
    Write-Host "Usage: rename_secret.ps1 <old_secret_name> <new_secret_name>"
    exit 1
}

# Get the value of the old secret
$value = az keyvault secret show --vault-name shine-keyvault --name $args[0] --query "value" --output tsv

# Set the value of the new secret
az keyvault secret set --vault-name shine-keyvault --name $args[1] --value "$value"