import { Command } from 'commander';
import * as utils from '../utils';

const SQL = () => `psql "${process.env.DATABASE_URL}" -c`;

export async function token(address: string, symbol: string) {
    console.log(`Setting token ${address} symbol to ${symbol}`);
    await utils.exec(`${SQL()} "UPDATE tokens SET symbol = '${symbol}' WHERE address = '${address}'"`);
}

export async function whitelist(address: string, add_or_remove: boolean) {
    var splitted_address = address.split(',');
    for (var addr of splitted_address) {
        console.log(`Setting ${addr} to ${add_or_remove}`);
        await utils.exec(
            `${SQL()} "UPDATE account_creates SET autorised = '${add_or_remove}' WHERE address = decode(substring('${addr}',3,length('${addr}')-2),'hex')"`
        );
    }
}

export const command = new Command('update').description('update information in the database');

command.command('token <address> <symbol>').description('update token symbol').action(token);
command
    .command('whitelist <address> <boolean>')
    .description('set specified address autorised or not')
    .action(whitelist);
