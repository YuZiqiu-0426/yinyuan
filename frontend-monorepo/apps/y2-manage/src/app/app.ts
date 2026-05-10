import { Component, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { SessionRenewScheduler } from './core/session-renew.scheduler';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet],
  templateUrl: './app.html',
})
export class App {
  constructor() {
    inject(SessionRenewScheduler);
  }
}
