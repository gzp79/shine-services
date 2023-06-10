$id=(fly machine list -a shine-db -j | jq '.[0].id').Trim('"')
fly machine start $id

$query = '.Machines | .[] | select(.id=="' + $id + '") | .checks | .[] | select(.name=="pg") | .status'
$status=(fly status -a shine-db -j | jq $query).Trim('"')
Write-Host Postgres status: $status

if ( $status -ne "passing" ) 
{
	fly pg restart -a shine-db
}
else {
	Write-Host Skipping postgres restart
}

fly proxy 15432:5432 -a shine-db

