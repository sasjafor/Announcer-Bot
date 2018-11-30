const token = process.env.AUTH_TOKEN;
const Discord = require('discord.js');
const speak = require('simple-tts-docker');
const exec = require('child_process').exec;
const fs = require('fs');
const https = require('https');
const client = new Discord.Client();
var names_list = [];
var conn = null;

client.on('ready', () => {
	console.log('I am ready!');
});

client.on('voiceStateUpdate', async (oldState, newState) => {
	let newUserChannel = newState.channel
	let oldUserChannel = oldState.channel
  
	let newMember = newState.member
	let oldMember = oldState.member

	if(newUserChannel && newUserChannel.joinable && !newMember.user.bot && !newState.mute && (!oldUserChannel || oldState.mute || oldState.channelID !== newState.channelID)) {
		// User Joins a voice channel
		if (!conn || conn.channel !== newUserChannel) {
			conn = await newUserChannel.join()
			.catch(console.error);
			//console.log("conn="+conn);
		}
		announce(newMember.displayName, conn, newUserChannel);
	} else if(oldUserChannel && (!newUserChannel || !newUserChannel.joinable)){
		// User leaves a voice channel
		var members = oldUserChannel.members;
		var leave = true;
	
		for (let [s,m] of members) {
			if(m !== oldMember && !m.user.bot) {
				leave = false;
			}
		}
	
		if (leave) {
			oldUserChannel.leave();
			conn = null;
		}
	}
});

client.on('message', async message => {
	if (message.channel.id == '511144158975623169') {
		if (message.content == '!newfile') {
			var audio_file = message.attachments.first();
			if (audio_file !== null && audio_file !== undefined) {
				var filename = audio_file.url.split('/').pop();
				filename = filename.replace(/_/g, ' ');
				console.log(filename);
				console.log(audio_file.url);
				if (filename.includes('.wav') || filename.includes('.m4a') || filename.includes('.mp3') || filename.includes('.ogg')) {
					var file = fs.createWriteStream("/config/audio/" + filename.slice(0,-3) + "wav");
					var request = https.get(audio_file.url, function(response) {
						response.pipe(file);
					});
				}
			}
		}
	}
});

client.login(token);

async function announce(name, connection, channel) {
	path = "/config/audio/" + name + ".wav";
	if (names_list.indexOf(name) <= -1) {
		fs.stat(path, function(err, stats) {
			if(err) {
				console.log('didn\'t find file:' + path);
				speak(name, {format:'wav', filename:'/config/audio/'+name});
				setTimeout(function() {names_list.push(name); }, 2000);
			} else {
				names_list.push(name);
				console.log("path="+path);
			}
		});
	}
	const intent = connection.play(path);
	intent.on('start', () => {
		console.log("playing " + name);
		console.log("list="+names_list);
    });
	intent.once('end', () => {
		intent.destroy();
	});
    intent.once('error', errWithFile => {
        console.log("file missing: " + errWithFile);
		intent.destroy();
    });
}
