import { Component } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { PERMISSIONS, buildSharedTestMessage } from '@y2/shared';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet],
  templateUrl: './app.html',
  styleUrl: './app.scss'
})
export class App {
  protected title = `y2-manage (${PERMISSIONS.length})`;
  protected sharedTestMessage = buildSharedTestMessage('y2-manage');
}
