
if ($args.Length -eq 1) {
    # list keys containing the search term
    $term = $args[0]
    az keyvault secret list --vault-name shine-keyvault --query "[?contains(name, '$term')].name"
} else  {
    az keyvault secret list --vault-name shine-keyvault --query "[].name"
}

